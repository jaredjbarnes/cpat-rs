//! The cursor's history (v12 `CursorHistory`): the recording parse, the
//! leaf matches, and the paths that name the pattern instance behind every
//! record.
//!
//! Ports `CursorHistory.test.ts` and `Cursor.test.ts`. Expected records come
//! from running v12 on the same grammars and inputs (byte offsets on ASCII
//! text; a TS node's inclusive `lastIndex` L is `end` L+1 here, an error's
//! `lastIndex` is `last`). v12 printed its paths by walking `parent` and
//! `children.indexOf` up to the pattern the test built, stepping over the
//! `Repeat` facade (see `Leaves`); they are the same paths.
//!
//! v12's cursor-movement tests have no Rust counterpart as such: the
//! half-open cursor has no `next`/`previous`/`currentChar`, it moves by
//! setting a position and snapping to grapheme boundaries. 'Empty Text',
//! 'Move Cursor', 'Text Information' and 'Text with Emojis' are mapped onto
//! those calls below.

mod common;

use common::*;
use cpat_core::{
    Choice, Cursor, HistoryRecord, Limits, Match, Meta, ParseError, Pattern, PatternPath,
    RepeatOptions, RightAssociated, TakeUntil, parse_log,
};

/// v12 `Cursor.test.ts`'s grammar:
///
/// ```text
/// john = "John"         doe = "Doe"
/// jane = "Jane"         smith = "Smith"
/// first-names = john | jane
/// last-names = doe | smith
/// space = " "
/// name = first-names + space + last-names
/// ```
fn name_grammar() -> Pattern {
    context(
        "name",
        seq(
            "name",
            vec![
                reference("first-names"),
                reference("space"),
                reference("last-names"),
            ],
        ),
        vec![
            lit("john", "John"),
            lit("jane", "Jane"),
            choice("first-names", vec![reference("john"), reference("jane")]),
            lit("doe", "Doe"),
            lit("smith", "Smith"),
            choice("last-names", vec![reference("doe"), reference("smith")]),
            lit("space", " "),
        ],
    )
}

fn path(steps: &[u32]) -> PatternPath {
    PatternPath::new(steps)
}

/// A match as v12's comparison printed it: `name@start..end/path`.
fn match_text(m: &Match) -> String {
    format!("{}@{}..{}{:?}", m.pattern.name, m.start, m.end, m.path)
}

/// An error as `name@start..last/path` (the path only when recorded).
fn error_text(e: &ParseError) -> String {
    let path = e.path.as_ref().map_or(String::new(), |p| format!("{p:?}"));
    format!("{}@{}..{}{path}", e.pattern.name, e.start, e.last)
}

/// A record as `M:` or `E:` and its text.
fn record_text(record: &HistoryRecord) -> String {
    match record {
        HistoryRecord::Match(m) => format!("M:{}", match_text(m)),
        HistoryRecord::Error(e) => format!("E:{}", error_text(e)),
    }
}

fn records_of(pattern: &mut Pattern, text: &str) -> Vec<String> {
    let result = pattern.exec_recording(text);
    assert!(result.pattern_error.is_none(), "{:?}", result.pattern_error);
    result.records().iter().map(record_text).collect()
}

fn leaves_of(result: &cpat_core::ParseResult<'_>) -> Vec<String> {
    result.leaf_matches().iter().map(match_text).collect()
}

/// Whether `path` leads, from `root`, to the pattern `meta` names.
fn replays(root: &Pattern, path: &PatternPath, meta: Meta) -> bool {
    root.at_path(path)
        .is_some_and(|found| found.id() == meta.id && found.name() == meta.name.as_str())
}

/// Every record of a recording exec, replayed on the root.
fn assert_every_record_replays(pattern: &mut Pattern, text: &str) {
    let result = pattern.exec_recording(text);
    assert!(!result.records().is_empty(), "{text:?} recorded nothing");
    for record in result.records() {
        let (path, meta) = match record {
            HistoryRecord::Match(m) => (&m.path, m.pattern),
            HistoryRecord::Error(e) => (e.path.as_ref().expect("recorded"), e.pattern),
        };
        assert!(
            replays(pattern, path, meta),
            "{text:?}: {} doesn't lead to its pattern",
            record_text(record)
        );
    }
    for leaf in result.leaf_matches() {
        assert!(replays(pattern, &leaf.path, leaf.pattern), "{text:?}");
    }
}

// ─── CursorHistory.test.ts ──────────────────────────────────────────────

#[test]
fn add_match() {
    let pattern = Meta::new("a");
    let mut cursor = Cursor::new("A");

    cursor.start_recording();
    cursor.record_match(pattern, 0, 1);

    assert!(cursor.is_recording());

    let leaf = cursor.leaf_match().unwrap();
    assert_eq!((leaf.pattern.id, leaf.start, leaf.end), (pattern.id, 0, 1));
    assert_eq!(leaf.path, PatternPath::root(), "no frames: the root");

    let root = cursor.root_match().unwrap();
    assert_eq!((root.pattern.id, root.start, root.end), (pattern.id, 0, 1));

    assert_eq!(cursor.matches().len(), 1);
    assert_eq!(cursor.matches()[0].pattern.id, pattern.id);

    cursor.stop_recording();

    assert!(!cursor.is_recording());
}

#[test]
fn add_error_at() {
    let pattern = Meta::new("a");
    let mut cursor = Cursor::new("A");

    cursor.start_recording();
    cursor.record_error(0, 0, pattern);

    let error = cursor.error().unwrap();
    assert_eq!(
        (error.start, error.last, error.pattern.id),
        (0, 0, pattern.id)
    );
    let first = &cursor.errors()[0];
    assert_eq!(
        (first.start, first.last, first.pattern.id),
        (0, 0, pattern.id)
    );

    cursor.stop_recording();
    cursor.resolve_error();

    assert!(!cursor.is_recording());
    assert!(cursor.error().is_none());

    let first = &cursor.errors()[0];
    assert_eq!(
        (first.start, first.last, first.pattern.id),
        (0, 0, pattern.id)
    );

    let furthest = cursor.furthest_error().unwrap();
    assert_eq!(
        (furthest.start, furthest.last, furthest.pattern.id),
        (0, 0, pattern.id)
    );
}

#[test]
fn stopping_keeps_what_was_recorded_and_adds_nothing() {
    let pattern = Meta::new("a");
    let mut cursor = Cursor::new("AB");

    cursor.start_recording();
    cursor.record_error(0, 0, pattern);
    cursor.stop_recording();
    cursor.record_error(1, 1, pattern);
    cursor.record_match(pattern, 0, 1);

    assert_eq!(cursor.errors().len(), 1);
    assert_eq!(cursor.records().len(), 1);
    assert!(cursor.matches().is_empty());
    assert_eq!(cursor.error().unwrap().last, 1, "the cursor still tracks");
}

// ─── Cursor.test.ts ─────────────────────────────────────────────────────

#[test]
fn empty_text() {
    // v12: `next` and `previous` stay at 0, `substring(0, 1)` is "".
    let cursor = Cursor::new("");

    assert_eq!(cursor.pos(), 0);
    assert!(cursor.at_end());
    assert_eq!(cursor.snap_forward(1), 0);
    assert_eq!(cursor.rest(), "");
}

#[test]
fn move_cursor() {
    // v12 moves an inclusive cursor onto characters (`moveTo`, `next`,
    // `previous`, `moveToLastChar`); here a position is set and snapped to
    // grapheme boundaries, and the "current character" is what follows it.
    let text = "Hello World!";
    let mut cursor = Cursor::new(text);

    cursor.set_pos(6);
    assert!(cursor.rest().starts_with('W'));

    cursor.set_pos(0);
    assert_eq!(cursor.snap_back(0), 0, "nothing before the first character");
    assert!(cursor.rest().starts_with('H'));

    cursor.set_pos(cursor.snap_forward(cursor.pos() + 1));
    assert!(cursor.rest().starts_with('e'));

    let last = cursor.snap_back(text.len() - 1);
    cursor.set_pos(last);
    assert_eq!(cursor.rest(), "!");

    cursor.set_pos(cursor.snap_back(last - 1));
    assert!(cursor.rest().starts_with('d'));
}

#[test]
fn error_handling() {
    let pattern = Meta::new("a");
    let mut cursor = Cursor::new("Hello World!");

    cursor.record_error(0, 0, pattern);

    assert!(cursor.has_error());
    let error = cursor.error().unwrap();
    assert_eq!(
        (error.start, error.last, error.pattern.id),
        (0, 0, pattern.id)
    );
    assert!(
        error.path.is_none(),
        "an ordinary parse builds no error path"
    );

    cursor.resolve_error();

    assert!(!cursor.has_error());
    assert!(cursor.error().is_none());
    let furthest = cursor.furthest_error().unwrap();
    assert_eq!(
        (furthest.start, furthest.last, furthest.pattern.id),
        (0, 0, pattern.id)
    );
}

#[test]
fn a_match_is_the_leaf_match_in_every_parse() {
    // v12's second 'Error Handling' test. v12 also sets `rootMatch` without
    // recording; here the root match is part of the recording only (it is
    // the last recorded match, DESIGN §7c), so it is `None`.
    let pattern = Meta::new("h");
    let mut cursor = Cursor::new("Hello World!");

    cursor.record_match(pattern, 0, 1);

    let leaf = cursor.leaf_match().unwrap();
    assert_eq!((leaf.pattern.id, leaf.start, leaf.end), (pattern.id, 0, 1));
    assert!(cursor.root_match().is_none());
}

#[test]
fn recording() {
    let mut cursor = Cursor::new("Hello World!");
    cursor.start_recording();

    assert!(cursor.is_recording());

    cursor.stop_recording();

    assert!(!cursor.is_recording());
}

#[test]
fn text_information() {
    // v12 `substring(0, 4)` is inclusive: bytes 0..5 here.
    let cursor = Cursor::new("Hello World!");

    assert_eq!(&cursor.text()[0..5], "Hello");
    assert_eq!(cursor.len(), 12);
    assert_eq!(cursor.text(), "Hello World!");
    assert_eq!(cursor.pos(), 0);
}

#[test]
fn records_all_matches() {
    let mut name = name_grammar();
    let result = name.exec_recording("John Doe");
    let records = result.records();

    assert_eq!(result.tree().unwrap().root().value(), "John Doe");
    let values: Vec<&str> = records
        .iter()
        .map(|record| match record {
            HistoryRecord::Match(m) => &"John Doe"[m.start..m.end],
            HistoryRecord::Error(_) => panic!("no errors on this input"),
        })
        .collect();
    assert_eq!(values, ["John", " ", "Doe", "John Doe"]);
}

#[test]
fn records_some_error_some_matches() {
    let mut name = name_grammar();
    let text = "John Smith";
    let result = name.exec_recording(text);
    let records = result.records();

    assert_eq!(result.tree().unwrap().root().value(), text);
    let value = |i: usize| match &records[i] {
        HistoryRecord::Match(m) => &text[m.start..m.end],
        HistoryRecord::Error(_) => "<error>",
    };
    assert_eq!(value(0), "John");
    assert_eq!(value(1), " ");
    let HistoryRecord::Error(error) = &records[2] else {
        panic!("records[2] is the error");
    };
    assert_eq!(
        (error.pattern.name.as_str(), error.start, error.last),
        ("doe", 5, 5)
    );
    assert_eq!(value(3), "Smith");
    assert_eq!(value(4), "John Smith");
}

#[test]
fn records_all_errors() {
    let mut name = name_grammar();
    let result = name.exec_recording("Jack Smith");
    let records = result.records();

    assert!(result.tree().is_none());
    let names: Vec<_> = records.iter().map(HistoryRecord::pattern_name).collect();
    assert!(matches!(records[0], HistoryRecord::Error(_)));
    assert!(matches!(records[1], HistoryRecord::Error(_)));
    assert!(matches!(records[2], HistoryRecord::Error(_)));
    assert_eq!(names[..3], ["john", "jane", "first-names"]);
}

#[test]
fn text_with_emojis() {
    // v12: the current character is the whole emoji, `next` lands on the
    // space, and the length is 9 UTF-16 units. Here it is 11 bytes.
    let cursor = Cursor::new("🔴 World!");

    let after_emoji = cursor.snap_forward(1);
    assert_eq!(&cursor.text()[..after_emoji], "🔴");
    assert!(cursor.text()[after_emoji..].starts_with(' '));
    assert_eq!(cursor.len(), 11);
}

#[test]
fn all_matched_patterns_tracks_patterns_after_recording() {
    let mut name = name_grammar();
    let result = name.exec_recording("John Doe");

    assert!(!result.matches().is_empty());
    assert_eq!(
        result.root_match().map(match_text).as_deref(),
        Some("name@0..8/0")
    );
}

// ─── The recording parse against v12 ───────────────────────────────────

#[test]
fn records_and_paths_match_v12() {
    // v12 (`exec(text, true)`, `cursor.records`), paths as described above.
    let mut name = name_grammar();
    assert_eq!(
        records_of(&mut name, "John Smith"),
        [
            "M:john@0..4/0/0/0/0/0",
            "M:space@4..5/0/1/0",
            "E:doe@5..5/0/2/0/0/0",
            "M:smith@5..10/0/2/0/1/0",
            "M:name@0..10/0",
        ]
    );
    assert_eq!(
        records_of(&mut name, "John  Doe"),
        [
            "M:john@0..4/0/0/0/0/0",
            "M:space@4..5/0/1/0",
            "E:doe@5..5/0/2/0/0/0",
            "E:smith@5..5/0/2/0/1/0",
            "E:last-names@5..5/0/2/0",
            "E:name@0..9/0",
        ]
    );
}

#[test]
fn an_ordinary_exec_keeps_no_error_list_and_a_recording_one_keeps_v12s() {
    let mut name = name_grammar();

    let plain = name.exec("Jack Smith");
    assert!(!plain.is_recording());
    assert!(plain.errors().is_empty());
    assert!(plain.records().is_empty());
    assert!(plain.root_match().is_none());
    assert_eq!(
        error_text(plain.furthest_error.as_ref().unwrap()),
        "jane@0..2"
    );

    let recorded = name.exec_recording("Jack Smith");
    assert!(recorded.is_recording());
    let errors: Vec<String> = recorded.errors().iter().map(error_text).collect();
    // v12: john@0..1, jane@0..2, first-names@0..0, then exec's summary,
    // which names the context's child as v12's `Context.exec` does.
    assert_eq!(
        errors,
        [
            "john@0..1/0/0/0/0/0",
            "jane@0..2/0/0/0/1/0",
            "first-names@0..0/0/0/0",
            "name@0..10/0",
        ]
    );
    assert_eq!(
        error_text(recorded.furthest_error.as_ref().unwrap()),
        "jane@0..2/0/0/0/1/0",
        "the furthest error is the recorded one, path and all"
    );
}

#[test]
fn empty_text_is_never_recorded() {
    // v12 returns before it starts recording.
    let mut name = name_grammar();
    let result = name.exec_recording("");

    assert!(!result.is_recording());
    assert!(result.records().is_empty());
}

#[test]
fn recording_changes_nothing_else() {
    let inputs = ["John Doe", "John Smith", "Jack Smith", "Jane", "John Do"];
    for text in inputs {
        let mut name = name_grammar();
        let plain = name.exec(text);
        let recorded = name.exec_recording(text);

        assert_eq!(plain.is_match(), recorded.is_match(), "{text}");
        assert_eq!(
            plain
                .error
                .as_ref()
                .map(|e| (e.start, e.last, e.pattern.id)),
            recorded
                .error
                .as_ref()
                .map(|e| (e.start, e.last, e.pattern.id)),
            "{text}"
        );
        assert_eq!(
            plain.furthest_error.as_ref().map(|e| (e.start, e.last)),
            recorded.furthest_error.as_ref().map(|e| (e.start, e.last)),
            "{text}"
        );
        assert_eq!(leaves_of(&plain), leaves_of(&recorded), "{text}");
    }
}

// ─── Paths replay ───────────────────────────────────────────────────────

/// `root = (expr, ",")+ ";" (t, "-"){1,3}` with
/// `expr = expr "*" expr | expr "+" expr | "-" expr | /[0-9]+/ | "(" expr ")"`
/// in a context: references, a Pratt choice (atoms and every operator
/// kind), and both repeats.
fn replay_grammar() -> Pattern {
    let e = || reference("expr");
    let expr = choice(
        "expr",
        vec![
            seq("mul", vec![e(), lit("times", "*"), e()]),
            seq("add", vec![e(), lit("plus", "+"), e()]),
            seq("neg", vec![lit("minus", "-"), e()]),
            seq("fact", vec![e(), lit("bang", "!")]),
            re("num", "[0-9]+"),
            seq("group", vec![lit("open", "("), e(), lit("close", ")")]),
        ],
    );
    let items = rep("items", e(), divided(lit("comma", ",")));
    let tail = rep(
        "tail",
        lit("t", "t"),
        RepeatOptions {
            divider: Some(lit("dash", "-")),
            min: 1,
            max: Some(3),
            trim_divider: true,
        },
    );
    context(
        "root",
        seq("root", vec![items, lit("semi", ";"), tail]),
        vec![expr],
    )
}

#[test]
fn recorded_paths_replay_through_references_pratt_choices_and_repeats() {
    let mut root = replay_grammar();
    let result = root.exec_recording("-1!+2,(3)*4;t-t-t");
    assert!(result.is_match());

    let find = |name: &str, start: usize| -> Match {
        result
            .matches()
            .iter()
            .find(|m| m.pattern.name.as_str() == name && m.start == start)
            .unwrap_or_else(|| panic!("no match {name}@{start}"))
            .clone()
    };

    // context/0 → root seq; child 0 is the unbounded repeat, whose child
    // is 0 and divider 1; the reference's instance is its child 0; the
    // Pratt choice numbers its classified alternatives in declaration
    // order (mul 0, add 1, neg 2, fact 3, num 4, group 5), an operator
    // being its sequence without the self-references.
    let cases = [
        (find("minus", 0), &[0, 0, 0, 0, 2, 0][..]),
        (find("bang", 2), &[0, 0, 0, 0, 3, 0]),
        (find("plus", 3), &[0, 0, 0, 0, 1, 0]),
        (find("num", 4), &[0, 0, 0, 0, 4]),
        (find("comma", 5), &[0, 0, 1]),
        (find("open", 6), &[0, 0, 0, 0, 5, 0]),
        (find("num", 7), &[0, 0, 0, 0, 5, 1, 0, 4]),
        (find("times", 9), &[0, 0, 0, 0, 0, 0]),
        // The bounded repeat's slots: t, -, t, -, t. (It records no match
        // of its own, as in v12.)
        (find("t", 12), &[0, 2, 0]),
        (find("dash", 15), &[0, 2, 3]),
        (find("t", 16), &[0, 2, 4]),
    ];
    for (found, steps) in cases {
        assert_eq!(found.path, path(steps), "{}", match_text(&found));
        assert!(
            replays(&root, &found.path, found.pattern),
            "{}",
            match_text(&found)
        );
    }

    assert_every_record_replays(&mut root, "-1!+2,(3)*4;t-t-t");
}

#[test]
fn every_record_replays_on_failures_too() {
    let mut root = replay_grammar();
    for text in ["1+", "(1;t", "1,;t", "1;t-t-t-t", "1;x", "1*-;t"] {
        assert_every_record_replays(&mut root, text);
    }
}

#[test]
fn paths_number_blocks_take_untils_lookaheads_and_optionals() {
    let mut pattern = seq(
        "s",
        vec![
            opt("o", lit("x", "x")),
            not("n", lit("y", "y")),
            block("b", "{", Some(re("content", "[a-z]+")), "}"),
            block("w", "(", None, ")"),
            take_until("tu", lit("end", ";")),
            lit("semi", ";"),
        ],
    );
    let result = pattern.exec_recording("x{ab}()zz;");
    assert!(result.is_match());
    let paths: Vec<String> = result.matches().iter().map(match_text).collect();

    assert_eq!(
        paths,
        [
            "x@0..1/0/0",
            "open@1..2/2/0",
            "content@2..4/2/1",
            "close@4..5/2/2",
            "b@1..5/2",
            "open@5..6/3/0",
            // No content: the close is the block's child 1.
            "close@6..7/3/1",
            "w@5..7/3",
            "end@9..10/4/0",
            "tu@7..9/4",
            "semi@9..10/5",
            "s@0..10/",
        ]
    );
    assert_every_record_replays(&mut pattern, "x{ab}()zz;");
    assert_every_record_replays(&mut pattern, "y{a1}");
}

#[test]
fn a_right_associated_marker_is_not_a_step() {
    let mut pattern = choice(
        "c",
        vec![RightAssociated::new(seq("ab", vec![lit("a", "a"), lit("b", "b")])).into()],
    );
    let result = pattern.exec_recording("ab");

    let a = &result.matches()[0];
    assert_eq!(match_text(a), "a@0..1/0/0");
    assert!(replays(&pattern, &a.path, a.pattern));
    // The path ends on the marked sequence, not the marker.
    let ab = &result.matches()[2];
    assert_eq!(match_text(ab), "ab@0..2/0");
    assert!(replays(&pattern, &ab.path, ab.pattern));
}

#[test]
fn a_path_into_nothing_finds_nothing() {
    let mut pattern = seq("s", vec![lit("a", "a"), reference("s")]);
    // The reference hasn't resolved: it has no instance to step into.
    assert!(pattern.at_path(&path(&[1, 0])).is_none());
    assert!(pattern.at_path(&path(&[2])).is_none());
    assert!(
        pattern.at_path(&path(&[0, 0])).is_none(),
        "a literal has no children"
    );

    let result = pattern.exec_recording("aa");
    let _ = result;
    assert_eq!(
        pattern.at_path(&path(&[1, 0])).map(Pattern::name),
        Some("s")
    );
    // A bounded repeat has exactly its slots.
    let bounded = rep(
        "r",
        lit("a", "a"),
        RepeatOptions {
            max: Some(2),
            ..RepeatOptions::default()
        },
    );
    assert_eq!(bounded.at_path(&path(&[1])).map(Pattern::name), Some("a"));
    assert!(bounded.at_path(&path(&[2])).is_none());
    // An unbounded one without a divider has only its child.
    let unbounded = rep("r", lit("a", "a"), RepeatOptions::default());
    assert!(unbounded.at_path(&path(&[1])).is_none());
}

#[test]
fn a_halt_is_recorded_under_the_pattern_that_halted() {
    // An unknown name, driven directly (exec would refuse it up front):
    // the reference records its own error.
    let mut pattern = seq("s", vec![lit("a", "a"), reference("missing")]);
    let mut cursor = Cursor::new("ab");
    cursor.start_recording();
    pattern.parse(&mut cursor);
    let last = cursor.errors().last().unwrap();
    assert_eq!(error_text(last), "missing@1..1/1");
    assert!(replays(&pattern, last.path.as_ref().unwrap(), last.pattern));

    // Too deep: the composite that was refused its frame records it.
    let mut nested = seq(
        "one",
        vec![seq("two", vec![seq("three", vec![lit("a", "a")])])],
    );
    let result = nested.exec_recording_with("a", Limits { max_depth: 2 });
    assert!(result.too_deep.is_some());
    let last = result
        .errors()
        .iter()
        .rev()
        .find(|e| e.pattern.name.as_str() == "three");
    assert_eq!(last.map(error_text).as_deref(), Some("three@0..0/0/0"));
}

#[test]
fn a_committed_operator_records_its_blocked_child_under_the_operator() {
    // `expr = expr ~/ "+" expr | /[0-9]+/` on "1": the operand ends the
    // text, the cut leading the infix fires, and the "+" it still needed is
    // the error. v12: `E:plus@1..1`, under the `add-delimiter` operator
    // (the choice's child 0), whose child 1 is the "+".
    let e = || reference("expr");
    let mut expr = choice(
        "expr",
        vec![
            seq("add", vec![e(), cut(), lit("plus", "+"), e()]),
            re("num", "[0-9]+"),
        ],
    );
    assert_eq!(
        records_of(&mut expr, "1"),
        ["M:num@0..1/1", "E:plus@1..1/0/1", "E:expr@0..1/"]
    );
    assert_every_record_replays(&mut expr, "1");
}

#[test]
fn a_blocked_lookahead_at_the_end_is_recorded_as_the_sequences_child() {
    // `("ab", !o)` with `o` an optional: at the end of the text `o` matches
    // the empty rest, so the lookahead fails, recorded by the sequence.
    let mut pattern = seq(
        "s",
        vec![lit("ab", "ab"), not("n", opt("o", lit("c", "c")))],
    );
    assert_eq!(
        records_of(&mut pattern, "ab"),
        ["M:ab@0..2/0", "E:n@2..2/1", "E:s@0..2/"]
    );
    assert_every_record_replays(&mut pattern, "ab");
}

// ─── The literal set under recording (closes the M5 hook) ───────────────

const KEYWORDS: [&str; 6] = ["if", "in", "int", "else", "elif", "x"];

fn keyword_literals() -> Vec<Pattern> {
    KEYWORDS
        .iter()
        .map(|k| lit(&format!("kw-{k}"), k))
        .collect()
}

#[test]
fn a_literal_set_records_what_the_ordered_loop_records() {
    let alternatives = keyword_literals();
    let mut fast = seq(
        "stmt",
        vec![
            Choice::new("keyword", alternatives.clone()).unwrap().into(),
            lit("semi", ";"),
        ],
    );
    let mut plain = seq(
        "stmt",
        vec![
            Choice::ordered("keyword", alternatives).unwrap().into(),
            lit("semi", ";"),
        ],
    );

    for text in ["int;", "inx;", "el;", "elif;", "x;", "y;", "if", "e"] {
        let a = fast.exec_recording(text);
        let b = plain.exec_recording(text);
        let errors = |r: &cpat_core::ParseResult<'_>| -> Vec<String> {
            r.errors().iter().map(error_text).collect()
        };
        let records = |r: &cpat_core::ParseResult<'_>| -> Vec<String> {
            r.records().iter().map(record_text).collect()
        };

        assert!(!errors(&a).is_empty(), "{text}");
        assert_eq!(errors(&a), errors(&b), "{text}");
        assert_eq!(records(&a), records(&b), "{text}");
        assert_eq!(leaves_of(&a), leaves_of(&b), "{text}");
    }
}

#[test]
fn a_recording_parse_tries_every_literal_the_loop_would() {
    // v12 `exec("inx", true)`: if, in and int are tried; `in` wins first.
    let mut keyword = choice("keyword", keyword_literals());
    parse_log::take_literals();

    let result = keyword.exec_recording("inx");

    assert_eq!(parse_log::take_literals(), ["if", "in"]);
    let errors: Vec<String> = result.errors().iter().map(error_text).collect();
    assert_eq!(errors, ["kw-if@0..1/0", "keyword@2..3/"]);

    keyword.exec("inx");
    assert_eq!(
        parse_log::take_literals(),
        ["in"],
        "the index parses the winner only"
    );
}

// ─── Leaf matches (v12 `leafMatches`) ───────────────────────────────────

#[test]
fn a_sibling_ending_at_the_same_spot_is_kept_in_front() {
    // c = ("a" "b" "x") | ("a" "b") on "ab". v12 `leafMatches`:
    // [b (two's), b (one's)]. Alternative one's `b` is furthest first;
    // two's `b` ends there too, and its parent (`two`) is no ancestor of
    // one's `b`, so it goes in front. `two` itself then ends there, but its
    // parent, the choice, is an ancestor of both: dropped.
    let mut c = choice(
        "c",
        vec![
            seq("one", vec![lit("a", "a"), lit("b", "b"), lit("x", "x")]),
            seq("two", vec![lit("a", "a"), lit("b", "b")]),
        ],
    );

    let result = c.exec("ab");

    assert_eq!(leaves_of(&result), ["b@1..2/1/1", "b@1..2/0/1"]);
    assert_eq!(
        result.leaf_match.as_ref().map(match_text).as_deref(),
        Some("b@1..2/0/1"),
        "the leaf match is the last"
    );
    assert_eq!(leaves_of(&c.exec_recording("ab")), leaves_of(&result));
}

#[test]
fn the_root_has_no_parent_so_it_is_kept_beside_its_last_leaf() {
    // root = c ";" (c as above) on "ab;". v12 `leafMatches`: [root, semi].
    // `semi` is furthest; `root` ends there too and has no parent.
    let mut root = seq(
        "root",
        vec![
            choice(
                "c",
                vec![
                    seq("one", vec![lit("a", "a"), lit("b", "b"), lit("x", "x")]),
                    seq("two", vec![lit("a", "a"), lit("b", "b")]),
                ],
            ),
            lit("semi", ";"),
        ],
    );

    assert_eq!(leaves_of(&root.exec("ab;")), ["root@0..3/", "semi@2..3/1"]);
    // v12 on "ab": [b (two's), b (one's)], the sequence failing after.
    assert_eq!(
        leaves_of(&root.exec("ab")),
        ["b@1..2/0/1/1", "b@1..2/0/0/1"]
    );
}

#[test]
fn a_repeat_at_the_root_is_not_kept_beside_its_leaf() {
    // v12 `leafMatches` for `a+` on "aa": [a]. v12's `Repeat` facade is the
    // inner repeat's parent and the leaf's ancestor, so the repeat's own
    // match is dropped, at the root too (a sequence there is kept, above).
    let mut repeat = rep("as", lit("a", "a"), RepeatOptions::default());

    assert_eq!(leaves_of(&repeat.exec("aa")), ["a@1..2/0"]);
}

#[test]
fn a_context_root_is_a_parent() {
    // v12's `Context.exec` runs its child, whose parent is the context, an
    // ancestor of every leaf: the child's match is dropped. v12 on
    // "John Doe": [doe].
    let mut name = name_grammar();

    assert_eq!(leaves_of(&name.exec("John Doe")), ["doe@5..8/0/2/0/0/0"]);
}

#[test]
fn an_ordinary_exec_gives_the_leaf_match_its_path() {
    // So an error message can start from the furthest match without a
    // recording parse.
    let mut root = replay_grammar();
    let result = root.exec("1+2;t-x");

    assert!(!result.is_recording());
    let leaf = result.leaf_match.clone().unwrap();
    assert_eq!(match_text(&leaf), "dash@5..6/0/2/1");
    assert!(replays(&root, &leaf.path, leaf.pattern));
}

#[test]
fn deviation_a_take_until_terminator_has_its_take_until_as_parent() {
    // v12's `TakeUntil` never makes itself its terminator's parent, so the
    // terminator has none and v12 keeps an extra leaf: on "<ab>",
    // [tag, close (the sequence's), close (the terminator's)]. Here the
    // terminator is the take-until's child 0, `tag` is its ancestor, and
    // the sequence's `close` is dropped like any match closing over a kept
    // leaf.
    let mut tag = seq(
        "tag",
        vec![
            lit("open", "<"),
            TakeUntil::new("name", lit("close", ">")).into(),
            lit("close", ">"),
        ],
    );

    assert_eq!(
        leaves_of(&tag.exec("<ab>")),
        ["tag@0..4/", "close@3..4/1/0"]
    );
}

#[test]
fn deviation_an_idle_bounded_repeat_records_its_first_idle_lap_only() {
    // `(x?){1,3} "b"` on "b". v12 walks all three slots, each a clone of
    // `x?` whose `x` fails and is recorded: E:x, E:x, E:x, M:b, M:s. Here
    // an idle lap ends the walk (every later lap would repeat it exactly,
    // DESIGN §10), so the failure is recorded once. The result, the error
    // and the furthest error are the same.
    let mut pattern = seq(
        "s",
        vec![
            rep(
                "r",
                opt("o", lit("x", "x")),
                RepeatOptions {
                    max: Some(3),
                    ..RepeatOptions::default()
                },
            ),
            lit("b", "b"),
        ],
    );

    assert_eq!(
        records_of(&mut pattern, "b"),
        ["E:x@0..0/0/0/0", "M:b@0..1/1", "M:s@0..1/"]
    );
}
