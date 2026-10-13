//! The selector language, in cpat, as v12 writes it in `selector_parser.ts`.

use std::cell::RefCell;

use super::error::SelectorError;
use crate::generate_error_message;
use crate::grammar::patterns;
use crate::pattern::Pattern;
use crate::tree::Tree;

/// v12's grammar text, with its template-literal escapes undone (`\\d` in
/// the TS source is `\d` in the text the grammar sees).
const SELECTOR_GRAMMAR: &str = r#"
number = /[+-]?(\d+(\.\d*)?|\.\d+)([eE][+-]?\d+)?/
spaces = /\s+/
single-quote-string-literal = /'(?:\\.|[^'\\])*'/
name = /[a-zA-Z_-]+[a-zA-Z0-9_-]*/
comma = /\s*,\s*/
wild-card = "*"
equal = "="
not-equal = "!="
starts-with = "^="
ends-with = "$="
contains = "*="
greater-than-or-equal = ">="
less-than-or-equal = "<="
greater-than = ">"
less-than = "<"
operators =      equal |
             not-equal |
           starts-with |
             ends-with |
              contains |
 greater-than-or-equal |
    less-than-or-equal |
          greater-than |
             less-than

attribute-name = name
value = name
attribute-value = single-quote-string-literal | number | value
attribute-selector = "[" + spaces? + attribute-name + spaces? + operators + spaces? + attribute-value + "]"

adjacent = spaces? + "+" + spaces?
after = spaces? + "~" + spaces?
direct-child = spaces? + ">" + spaces?
descendant = spaces

combinators =  adjacent | after | direct-child | descendant
name-selector = name-selector-expression + attribute-selector
name-selector-expression = name-selector | name
node-selector = wild-card | attribute-selector | name-selector-expression
or-selector = (node-selector, comma){2}

selector-expression = expression + combinators + expression
expression = selector-expression | or-selector | node-selector
"#;

thread_local! {
    // Built once per thread: building a grammar costs far more than parsing a
    // selector with it. A `Pattern` holds `Rc`s, so it can't be shared
    // across threads.
    static SELECTOR_PARSER: RefCell<Pattern> = RefCell::new(build());
}

fn build() -> Pattern {
    let mut rules = patterns(SELECTOR_GRAMMAR)
        .expect("the selector grammar is fixed text, and a test builds it");
    rules
        .remove("expression")
        .expect("the selector grammar defines `expression`")
}

/// The selector's syntax tree, or v12's `[Invalid Selector]` error.
///
/// The parser is borrowed only while it parses: compiling an or-selector
/// parses its parts again (as v12 does), so the borrow must be over by then.
pub(crate) fn parse_selector(text: &str) -> Result<Tree<'_>, SelectorError> {
    SELECTOR_PARSER.with(|parser| {
        let mut parser = parser.borrow_mut();
        let mut result = parser.exec(text);
        match result.tree.take() {
            Some(tree) => Ok(tree),
            None => Err(SelectorError {
                message: format!(
                    "[Invalid Selector] {}",
                    generate_error_message(&parser, &result)
                ),
            }),
        }
    })
}
