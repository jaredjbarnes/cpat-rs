//! The cpat grammar, written by hand with the pattern API: a port of v12's
//! `grammar/patterns/grammar.ts`, pattern for pattern, with the same names
//! and structure (DESIGN §8). Every `Expression` there is a `Choice` here.
//!
//! The builder reads the trees this grammar makes by node name, so a name
//! changed here is a change to every grammar file's meaning.

use crate::pattern::Pattern;
use crate::patterns::{
    Choice, Literal, Optional, Reference, Regex, Repeat, RepeatOptions, Sequence,
};

/// The grammar `.cpat` files are written in. Its root is the sequence
/// `cpat`: an optional `syntax` line, then statements one per line.
///
/// `patternExpr`, the right-hand side of a rule, is a choice in Pratt mode:
/// `!p` is a prefix operator, `p?` and `p right` are postfix operators, and
/// `a + b`, `a | b` and the removed `a <|> b` are infix operators, binding
/// in that order (a sequence binds tighter than a choice).
pub fn cpat_grammar() -> Pattern {
    Rules::new().cpat()
}

/// A static grammar that doesn't build is a bug in this file, never in
/// anyone's input.
const VALID: &str = "the cpat grammar is valid";

fn lit(name: &str, token: &str) -> Pattern {
    Literal::new(name, token).expect(VALID).into()
}

fn re(name: &str, source: &str) -> Pattern {
    Regex::new(name, source).expect(VALID).into()
}

fn seq(name: &str, children: Vec<Pattern>) -> Pattern {
    Sequence::new(name, children).expect(VALID).into()
}

fn choice(name: &str, alternatives: Vec<Pattern>) -> Pattern {
    Choice::new(name, alternatives).expect(VALID).into()
}

fn opt(name: &str, child: Pattern) -> Pattern {
    Optional::new(name, child).into()
}

fn reference(name: &str) -> Pattern {
    Reference::new(name).into()
}

/// One or more `child`, with `divider` between them.
fn divided(name: &str, child: Pattern, divider: Pattern, trim_divider: bool) -> Pattern {
    let options = RepeatOptions {
        divider: Some(divider),
        trim_divider,
        ..RepeatOptions::default()
    };
    Repeat::new(name, child, options).into()
}

/// The patterns v12 exports one by one and reuses (cloned, so they share
/// ids, as v12's composites clone their children).
struct Rules {
    ls: Pattern,
    comma: Pattern,
    name: Pattern,
    literal: Pattern,
    optional_ws: Pattern,
    optional_ls: Pattern,
    open_paren: Pattern,
    close_paren: Pattern,
    open_square_bracket: Pattern,
    close_square_bracket: Pattern,
    open_bracket: Pattern,
    close_bracket: Pattern,
    assign: Pattern,
    new_line: Pattern,
}

impl Rules {
    fn new() -> Rules {
        let ws = re("ws", r"\s+");
        let ls = re("ls", r"[ \t]+");
        Rules {
            optional_ws: opt("optionalWS", ws),
            optional_ls: opt("optionalLS", ls.clone()),
            ls,
            comma: re(",", r"\s*[,]\s*"),
            name: re("name", "[a-zA-Z_-]+[a-zA-Z0-9_-]*"),
            literal: re("literal", r#""(?:\\.|[^"\\])*""#),
            open_paren: lit("(", "("),
            close_paren: lit(")", ")"),
            open_square_bracket: lit("[", "["),
            close_square_bracket: lit("]", "]"),
            open_bracket: lit("{", "{"),
            close_bracket: lit("}", "}"),
            assign: lit("assign", "="),
            new_line: re("newLine", r"\s*(\r\n|\r|\n)\s*"),
        }
    }

    fn pattern_name(&self) -> Pattern {
        self.name.clone_named("patternName")
    }

    fn cpat(&self) -> Pattern {
        let statements = Repeat::new(
            "statements",
            self.statement(),
            RepeatOptions {
                divider: Some(self.new_line.clone()),
                ..RepeatOptions::default()
            },
        );
        seq(
            "cpat",
            vec![
                self.optional_ws.clone(),
                opt("optionalSyntaxStatement", self.syntax_statement()),
                self.optional_ws.clone(),
                opt("optionalStatements", statements.into()),
                self.optional_ws.clone(),
            ],
        )
    }

    fn syntax_statement(&self) -> Pattern {
        seq(
            "syntaxStatement",
            vec![
                lit("syntax", "syntax"),
                self.optional_ws.clone(),
                re("syntaxVersion", r"\S*"),
            ],
        )
    }

    fn statement(&self) -> Pattern {
        choice(
            "statement",
            vec![
                self.use_params_statement(),
                self.import_statement(),
                self.pattern_assignment(),
                self.decoration_statement(),
                self.pattern_name().clone_named("exportPattern"),
                re("comment", r"#[^\n\r]*"),
            ],
        )
    }

    // ─── Rules ──────────────────────────────────────────────────────────

    fn pattern_assignment(&self) -> Pattern {
        seq(
            "patternAssignment",
            vec![
                self.pattern_name(),
                self.optional_ws.clone(),
                self.assign.clone(),
                self.optional_ws.clone(),
                self.pattern_expr(),
            ],
        )
    }

    fn pattern_expr(&self) -> Pattern {
        let expr = || reference("patternExpr");
        let ws = || self.optional_ws.clone();
        let ls = || self.optional_ls.clone();
        let infix =
            |name: &str, operator: Pattern| seq(name, vec![expr(), ws(), operator, ws(), expr()]);

        let seq_separator = choice(
            "seqSeparator",
            // `~/` sequences like `+` and commits the enclosing choice once
            // the parse passes it (see `Cut`).
            vec![lit("~/", "~/"), lit("+", "+")],
        );

        choice(
            "patternExpr",
            vec![
                seq("notExpr", vec![lit("not", "!"), ls(), expr()]),
                seq("optionalExpr", vec![expr(), ls(), lit("?", "?")]),
                seq(
                    "rightAssociationExpr",
                    vec![expr(), ls(), lit("right", "right")],
                ),
                infix("sequenceExpr", seq_separator),
                infix("optionsExpr", lit("|", "|")),
                // `<|>` (greedy options) was removed in v12. It is still
                // recognized here ONLY so the builder can reject it with a
                // message that teaches the fix, instead of a generic syntax
                // error. Nothing is ever built from it.
                infix("removedGreedyOptionsExpr", lit("<|>", "<|>")),
                self.repeat_expr(),
                seq(
                    "patternGroupExpr",
                    vec![
                        self.open_paren.clone(),
                        ws(),
                        expr(),
                        ws(),
                        self.close_paren.clone(),
                    ],
                ),
                seq(
                    "takeUntilExpr",
                    vec![
                        lit("anyChar", "?"),
                        ls(),
                        lit("upTo", "->"),
                        ls(),
                        lit("wall", "|"),
                        ls(),
                        expr(),
                    ],
                ),
                self.block_expr(),
                self.literal.clone(),
                re("regex", r"\/(?:\\.|[^\/\n\r])*\/"),
                self.name.clone_named("patternIdentifier"),
            ],
        )
    }

    fn repeat_expr(&self) -> Pattern {
        let ws = || self.optional_ws.clone();
        let integer = re("integer", "[1-9][0-9]*|[0]");
        let integer = || opt("optionalInteger", integer.clone());
        let repeat_bounds = seq(
            "repeatBounds",
            vec![
                self.open_bracket.clone(),
                ws(),
                integer(),
                ws(),
                opt("optionalComma", self.comma.clone()),
                ws(),
                integer(),
                ws(),
                self.close_bracket.clone(),
            ],
        );
        let repeat_options = choice(
            "repeatOptions",
            vec![lit("oneOrMore", "+"), lit("zeroOrMore", "*"), repeat_bounds],
        );
        let delimiter = seq(
            "delimiter",
            vec![
                self.comma.clone(),
                reference("patternExpr"),
                ws(),
                opt("optionalTrim", lit("trim", "trim")),
            ],
        );
        seq(
            "repeatExpr",
            vec![
                self.open_paren.clone(),
                ws(),
                reference("patternExpr"),
                ws(),
                opt("optionalDelimiter", delimiter),
                ws(),
                self.close_paren.clone(),
                repeat_options,
            ],
        )
    }

    fn block_expr(&self) -> Pattern {
        let ws = || self.optional_ws.clone();
        let block_delimiter = seq(
            "blockDelimiter",
            vec![
                self.open_square_bracket.clone(),
                ws(),
                self.literal.clone(),
                ws(),
                self.close_square_bracket.clone(),
            ],
        );
        let block_content = choice(
            "blockContent",
            vec![lit("wildcard", "..."), reference("patternExpr")],
        );
        seq(
            "blockExpr",
            vec![
                block_delimiter.clone(),
                ws(),
                block_content,
                ws(),
                block_delimiter,
            ],
        )
    }

    // ─── Imports and params ─────────────────────────────────────────────

    fn use_params_statement(&self) -> Pattern {
        let param_default = seq(
            "paramDefault",
            vec![
                self.optional_ls.clone(),
                self.assign.clone(),
                self.optional_ls.clone(),
                self.name.clone_named("defaultParamName"),
            ],
        );
        let param_name_with_default = seq(
            "paramNameWithDefault",
            vec![
                self.pattern_name(),
                opt("optionalParamDefault", param_default),
            ],
        );
        seq(
            "useParamsStatement",
            vec![
                lit("useParams", "use params"),
                self.optional_ls.clone(),
                self.open_bracket.clone(),
                self.optional_ws.clone(),
                divided(
                    "useParamPatterns",
                    param_name_with_default,
                    self.comma.clone(),
                    true,
                ),
                self.optional_ws.clone(),
                self.close_bracket.clone(),
            ],
        )
    }

    fn import_statement(&self) -> Pattern {
        let ls = || self.optional_ls.clone();
        let import_alias = seq(
            "importAlias",
            vec![
                self.pattern_name(),
                self.ls.clone(),
                lit("as", "as"),
                self.ls.clone(),
                self.name.clone_named("importNameAlias"),
            ],
        );
        let import_name_or_alias =
            choice("importNameOrAlias", vec![import_alias, self.pattern_name()]);
        let imported_patterns = seq(
            "importedPatterns",
            vec![
                self.open_bracket.clone(),
                self.optional_ws.clone(),
                divided(
                    "patternNames",
                    import_name_or_alias,
                    self.comma.clone(),
                    true,
                ),
                self.optional_ws.clone(),
                self.close_bracket.clone(),
            ],
        );
        seq(
            "importStatement",
            vec![
                lit("import", "import"),
                ls(),
                imported_patterns,
                ls(),
                lit("from", "from"),
                ls(),
                self.literal.clone_named("resource"),
                ls(),
                opt("optionalWithParamsExpr", self.with_params_expr()),
            ],
        )
    }

    fn with_params_expr(&self) -> Pattern {
        let with_param_statement = choice(
            "withParamStatement",
            vec![
                reference("patternAssignment"),
                self.pattern_name().clone_named("withParamExportPattern"),
            ],
        );
        let with_param_statements = Repeat::new(
            "withParamStatements",
            with_param_statement,
            RepeatOptions {
                divider: Some(self.new_line.clone()),
                trim_divider: true,
                ..RepeatOptions::default()
            },
        );
        seq(
            "withParamsExpr",
            vec![
                lit("withParams", "with params"),
                self.optional_ls.clone(),
                self.open_bracket.clone(),
                self.optional_ws.clone(),
                with_param_statements.into(),
                self.optional_ws.clone(),
                self.close_bracket.clone(),
            ],
        )
    }

    // ─── Decorators ─────────────────────────────────────────────────────

    fn decoration_statement(&self) -> Pattern {
        let at = lit("@", "@");
        let at = || at.clone();
        let decoration_name = self.name.clone_named("decorationName");
        let decoration_name = || decoration_name.clone();
        let ws = || self.optional_ws.clone();
        let method = seq(
            "methodDecorationStatement",
            vec![
                at(),
                ws(),
                decoration_name(),
                ws(),
                self.open_paren.clone(),
                ws(),
                opt("optionalJsonValue", self.json_value()),
                ws(),
                self.close_paren.clone(),
            ],
        );
        let bare = seq(
            "nameDecorationStatement",
            vec![at(), ws(), decoration_name()],
        );
        choice("decorationStatement", vec![method, bare])
    }

    fn json_value(&self) -> Pattern {
        let ws = || self.optional_ws.clone();
        let json_array = seq(
            "jsonArray",
            vec![
                self.open_square_bracket.clone(),
                ws(),
                divided(
                    "jsonArrayItems",
                    reference("jsonValue"),
                    self.comma.clone(),
                    true,
                ),
                ws(),
                self.close_square_bracket.clone(),
            ],
        );
        let json_object_property = seq(
            "jsonObjectProperty",
            vec![
                self.literal.clone_named("jsonObjectPropertyName"),
                ws(),
                lit(":", ":"),
                ws(),
                reference("jsonValue"),
            ],
        );
        let json_object = seq(
            "jsonObject",
            vec![
                self.open_bracket.clone(),
                ws(),
                opt(
                    "optionalJsonObjectProperties",
                    divided(
                        "jsonObjectProperties",
                        json_object_property,
                        self.comma.clone(),
                        true,
                    ),
                ),
                ws(),
                self.close_bracket.clone(),
            ],
        );
        choice(
            "jsonValue",
            vec![
                self.literal.clone_named("jsonString"),
                re("jsonNumber", r"-?\d+(\.\d+)?"),
                choice(
                    "jsonBoolean",
                    vec![lit("trueLiteral", "true"), lit("falseLiteral", "false")],
                ),
                lit("jsonNull", "null"),
                json_array,
                json_object,
            ],
        )
    }
}
