//! What milestone 15 added to the core for custom patterns written in JS:
//! a way for a custom pattern whose own code failed to stop the parse
//! (`Cursor::halt_custom`), the cursor id a custom pattern keys per-parse
//! work on, and the recording parse a `Suggestion` keeps (v12's cursor).

mod common;

use std::cell::RefCell;
use std::rc::Rc;

use common::*;
use cpat_core::{
    AutoComplete, AutoCompleteOptions, Cursor, Custom, CustomParse, Meta, NodeId, Pattern,
    PatternError,
};

/// Each call's position, in order.
type Calls = Rc<RefCell<Vec<usize>>>;

/// A custom pattern that notes where it was asked, then halts the parse.
fn failing(name: &str) -> (Pattern, Calls) {
    #[derive(Clone)]
    struct Failing(Calls);

    impl CustomParse for Failing {
        fn parse(&mut self, meta: Meta, cursor: &mut Cursor<'_>) -> Option<NodeId> {
            self.0.borrow_mut().push(cursor.pos());
            cursor.halt_custom(meta, "boom".to_string());
            None
        }

        fn clone_box(&self) -> Box<dyn CustomParse> {
            Box::new(self.clone())
        }
    }

    let calls = Calls::default();
    let custom = Custom::new(name, Box::new(Failing(Rc::clone(&calls))));
    (custom.into(), calls)
}

/// A custom pattern that notes where it was asked and fails to match.
fn spy(name: &str) -> (Pattern, Calls) {
    #[derive(Clone)]
    struct Spy(Calls);

    impl CustomParse for Spy {
        fn parse(&mut self, meta: Meta, cursor: &mut Cursor<'_>) -> Option<NodeId> {
            self.0.borrow_mut().push(cursor.pos());
            cursor.record_error(cursor.pos(), cursor.pos(), meta);
            None
        }

        fn clone_box(&self) -> Box<dyn CustomParse> {
            Box::new(self.clone())
        }
    }

    let calls = Calls::default();
    let custom = Custom::new(name, Box::new(Spy(Rc::clone(&calls))));
    (custom.into(), calls)
}

#[test]
fn a_halted_custom_pattern_ends_the_parse_with_its_reason() {
    let (broken, _) = failing("broken");
    let mut pattern = seq("s", vec![lit("a", "a"), broken]);

    let result = pattern.exec("ab");

    assert_eq!(
        result.pattern_error,
        Some(PatternError::CustomFailed {
            name: "broken".to_string(),
            reason: "boom".to_string(),
        })
    );
    assert_eq!(
        result.pattern_error.as_ref().unwrap().to_string(),
        "custom pattern `broken` failed: boom"
    );
    assert!(result.tree().is_none());
}

#[test]
fn a_halt_is_not_a_failure_to_match_so_no_alternative_runs_after_it() {
    let (broken, broken_calls) = failing("broken");
    let (other, other_calls) = spy("other");
    let mut pattern = choice("c", vec![broken, other]);

    let result = pattern.exec("x");

    assert!(result.pattern_error.is_some());
    assert_eq!(*broken_calls.borrow(), [0]);
    assert!(other_calls.borrow().is_empty());
}

#[test]
fn every_cursor_has_its_own_id_even_over_the_same_text() {
    let first = Cursor::new("same");
    let second = Cursor::new("same");

    assert_ne!(first.id(), second.id());
}

#[test]
fn a_suggestion_keeps_the_recording_parse_it_came_from() {
    let name = seq(
        "name",
        vec![
            lit("john", "John"),
            lit("space", " "),
            choice("last", vec![lit("doe", "Doe"), lit("smith", "Smith")]),
        ],
    );
    let mut auto_complete = AutoComplete::new(name, AutoCompleteOptions::default()).unwrap();

    // v12's cursor after `suggestFor("John D")` (checked with tsx).
    let suggestion = auto_complete.suggest_for("John D");
    let parse = suggestion
        .parse
        .as_ref()
        .expect("text that isn't empty is parsed");

    assert!(parse.is_recording());
    let failed: Vec<&str> = parse
        .errors()
        .iter()
        .map(|e| e.pattern.name.as_str())
        .collect();
    assert_eq!(failed, ["doe", "smith", "last"]);
    let matched: Vec<&str> = parse
        .matches()
        .iter()
        .map(|m| m.pattern.name.as_str())
        .collect();
    assert_eq!(matched, ["john", "space"]);
    assert!(auto_complete.suggest_for("").parse.is_none());
}
