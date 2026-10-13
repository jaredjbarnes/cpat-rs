use super::*;

fn rust(source: &str) -> String {
    js_to_rust(source).unwrap().rust
}

fn dialect(source: &str) -> Dialect {
    js_to_rust(source).unwrap().dialect
}

fn syntax(source: &str) -> &'static str {
    match js_to_rust(source) {
        Err(TranslateError::Syntax(reason)) => reason,
        other => panic!("{source}: expected a syntax error, got {other:?}"),
    }
}

fn unsupported(source: &str) -> &'static str {
    match js_to_rust(source) {
        Err(TranslateError::Unsupported(reason)) => reason,
        other => panic!("{source}: expected unsupported, got {other:?}"),
    }
}

const LINE_BREAKS: &str = r"\x{A}\x{D}\x{2028}-\x{2029}";
const SPACES: &str = r"\x{9}-\x{D}\x{20}\x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}-\x{2029}\x{202F}\x{205F}\x{3000}\x{FEFF}";

#[test]
fn spells_the_dot_without_line_terminators() {
    assert_eq!(rust("a.b"), format!("a[^{LINE_BREAKS}]b"));
    assert_eq!(rust("(?s:.)"), "(?:(?s:.))");
}

#[test]
fn spells_whitespace_as_js_does() {
    assert_eq!(rust(r"\s"), format!("[{SPACES}]"));
    assert_eq!(rust(r"\S"), format!("[^{SPACES}]"));
    assert_eq!(rust(r"[\s,]"), format!("[{SPACES},]"));
    assert_eq!(rust(r"[^\S]"), format!("[^[^{SPACES}]]"));
}

#[test]
fn spells_ascii_classes_out() {
    assert_eq!(rust(r"\d+"), "(?:[0-9])+");
    assert_eq!(rust(r"[\d_]"), r"[0-9_]");
    assert_eq!(rust(r"\w"), "[0-9A-Z_a-z]");
    assert_eq!(rust(r"[^\W]"), "[^[^0-9A-Z_a-z]]");
    assert_eq!(rust(r"\bx"), r"(?-u:\b)x");
}

#[test]
fn spells_ascii_word_boundaries_with_lookaround_for_the_fancy_engine() {
    let word = "[0-9A-Z_a-z]";
    assert_eq!(
        rust(r"(?=x)\bx\B"),
        format!(
            "(?=x)(?:(?<={word})(?!{word})|(?<!{word})(?={word}))x\
             (?:(?<={word})(?={word})|(?<!{word})(?!{word}))"
        )
    );
}

#[test]
fn decodes_character_escapes() {
    assert_eq!(rust(r"a\cJb"), r"a\x{A}b");
    assert_eq!(rust(r"\cj"), r"\x{A}");
    assert_eq!(rust(r"\v\f\0\t"), r"\x{B}\x{C}\x{0}\x{9}");
    assert_eq!(rust(r"\x41\u0042\u{43}\u00e9"), "ABCé");
    assert_eq!(rust(r"\uD83D\uDE00"), r"\x{1F600}");
    assert_eq!(rust(r"[\uD83D\uDE00]"), r"[\x{1F600}]");
    assert_eq!(rust(r"a\/b"), "a/b");
    assert_eq!(rust(r"\.\*\{"), r"\.\*\{");
    assert_eq!(rust(r"[\b\-]"), r"[\x{8}\-]");
}

#[test]
fn a_lone_surrogate_matches_nothing() {
    assert_eq!(rust(r"\uD800"), r"[^\x{0}-\x{10FFFF}]");
    assert_eq!(rust(r"[\uD800a]"), "[a]");
    assert_eq!(
        rust(r"[\u0000-\uFFFF]"),
        r"[\x{0}-\x{D7FF}\x{E000}-\x{FFFF}]"
    );
}

#[test]
fn escapes_what_rust_reads_as_class_syntax() {
    assert_eq!(rust(r"[a&&b]"), r"[a\&\&b]");
    assert_eq!(rust(r"[[]"), r"[\[]");
    assert_eq!(rust(r"[~]"), r"[\~]");
    assert_eq!(rust(r"[a-]"), r"[a\-]");
    assert_eq!(rust(r"[a-c-e]"), r"[a-c\-e]");
}

#[test]
fn rewrites_js_only_forms() {
    assert_eq!(rust(r"[^]"), "(?s:.)");
    assert_eq!(rust(r"[]"), r"[^\x{0}-\x{10FFFF}]");
    assert_eq!(rust(r"(?<year>\d)"), "([0-9])");
}

#[test]
fn spells_modifier_groups() {
    assert_eq!(rust("(?i:a)"), "(?i:a)");
    assert_eq!(rust("(?i:(?-i:a))"), "(?i:(?-i:a))");
    // `m` and `s` are spelled into `^`, `$` and `.`; Rust never sees them.
    assert_eq!(rust("(?s-i:a)"), "(?:a)");
    assert_eq!(
        rust("(?m:^a$)"),
        format!("(?:(?:^|(?<=[{LINE_BREAKS}]))a(?:$|(?=[{LINE_BREAKS}])))")
    );
    assert_eq!(dialect("(?m:^a)"), Dialect::Fancy);
    assert_eq!(dialect("(?i:a)"), Dialect::Fast);
    assert_eq!(dialect(r"(?i:a\b)"), Dialect::Fancy);
}

#[test]
fn reads_a_backreference_only_once_its_group_has_matched() {
    assert_eq!(rust(r"(a)?b\1"), r"(?:(a))?b(?(1)\1|)");
    assert_eq!(rust(r"\1(a)"), r"(?(1)\1|)(a)");
    assert_eq!(rust(r"(?<x>a)\k<x>"), r"(a)(?(1)\1|)");
    assert_eq!(
        rust(r"(?:(?<y>a)|(?<y>b))\k<y>"),
        r"(?:(a)|(b))(?(1)\1|(?(2)\2|))"
    );
    // Inside its own group a backreference reads nothing in JS.
    assert_eq!(rust(r"(a\1)"), "(a)");
    assert_eq!(dialect(r"(a)\1"), Dialect::Fancy);
}

#[test]
fn keeps_backreferences_every_repetition_sets_first() {
    assert_eq!(rust(r#"(?:(["'])x\1)+"#), r#"(?:(?:(["'])x(?(1)\1|)))+"#);
    assert_eq!(rust(r"(?:(a)b)+\1"), r"(?:(?:(a)b))+(?(1)\1|)");
}

#[test]
fn refuses_backreferences_a_repetition_resets() {
    // JS clears group 1 each round; fancy-regex keeps the last round's.
    unsupported(r"(?:(a)|b)+\1");
    unsupported(r"(?:(a)|b\1)+");
    unsupported(r"(?:\1(a))+");
    // An empty round is dropped in JS and kept in fancy-regex.
    unsupported(r"(?:(a?))*b\1");
    unsupported(r"(a)(?<=\1)");
}

#[test]
fn checks_property_names_as_js_spells_them() {
    assert_eq!(rust(r"\p{L}\P{Lu}"), r"\p{L}\P{Lu}");
    assert_eq!(
        rust(r"[\p{Script=Greek}\p{scx=Latn}]"),
        r"[\p{Script=Greek}\p{scx=Latn}]"
    );
    assert_eq!(
        rust(r"\p{Alphabetic}\p{ASCII}\p{Any}"),
        r"\p{Alphabetic}\p{ASCII}\p{Any}"
    );
    assert_eq!(syntax(r"\p{letter}"), "Invalid property name");
    assert_eq!(syntax(r"\pL"), "Invalid property name");
    assert_eq!(syntax(r"\p{sc=latin}"), "Invalid property name");
    assert_eq!(syntax(r"\p{Script=L}"), "Invalid property name");
    assert_eq!(
        syntax(r"[\p{x}]"),
        "Invalid property name in character class"
    );
}

#[test]
fn every_js_property_name_compiles_in_rust() {
    use js_property_names::{BINARY_PROPERTIES, GENERAL_CATEGORY_VALUES, SCRIPT_VALUES};
    let names = GENERAL_CATEGORY_VALUES
        .iter()
        .flat_map(|v| {
            [
                v.to_string(),
                format!("gc={v}"),
                format!("General_Category={v}"),
            ]
        })
        .chain(BINARY_PROPERTIES.iter().map(|v| v.to_string()))
        .chain(SCRIPT_VALUES.iter().flat_map(|v| {
            ["sc", "Script", "scx", "Script_Extensions"].map(|p| format!("{p}={v}"))
        }));
    let failures: Vec<String> = names
        .map(|name| format!(r"\p{{{name}}}"))
        .filter(|source| match js_to_rust(source) {
            Ok(translation) => regex::Regex::new(&translation.rust).is_err(),
            Err(error) => !matches!(error, TranslateError::Unsupported(_)),
        })
        .collect();
    assert_eq!(failures, Vec::<String>::new());
}

#[test]
fn surrogate_categories_match_nothing_and_rust_less_properties_are_refused() {
    assert_eq!(rust(r"\p{Cs}"), r"[^\x{0}-\x{10FFFF}]");
    assert_eq!(rust(r"\P{gc=Surrogate}"), "(?s:.)");
    assert_eq!(rust(r"[a\p{Surrogate}]"), "[a]");
    assert_eq!(rust(r"[^\P{Cs}]"), r"[^\x{0}-\x{D7FF}\x{E000}-\x{10FFFF}]");
    unsupported(r"\p{CWKCF}");
    unsupported(r"\p{Script=Unknown}");
    unsupported(r"\p{scx=Zzzz}");
}

#[test]
fn property_tables_are_sorted_for_binary_search() {
    use js_property_names::{BINARY_PROPERTIES, GENERAL_CATEGORY_VALUES, SCRIPT_VALUES};
    for table in [GENERAL_CATEGORY_VALUES, BINARY_PROPERTIES, SCRIPT_VALUES] {
        assert!(table.windows(2).all(|w| w[0] < w[1]));
    }
}

#[test]
fn refuses_what_js_u_mode_refuses() {
    let cases = [
        (r"[\d-z]", "Invalid character class"),
        (r"[a-\d]", "Invalid character class"),
        (r"[b-a]", "Range out of order in character class"),
        (r"[a", "Unterminated character class"),
        (r"(a", "Unterminated group"),
        (r"a)", "Unmatched ')'"),
        (r"*", "Nothing to repeat"),
        (r"a**", "Nothing to repeat"),
        (r"a*+", "Nothing to repeat"),
        (r"a{1}{2}", "Nothing to repeat"),
        (r"\b*", "Nothing to repeat"),
        (r"{", "Lone quantifier brackets"),
        (r"}", "Lone quantifier brackets"),
        (r"]", "Lone quantifier brackets"),
        (r"a{2", "Incomplete quantifier"),
        (r"a{,3}", "Incomplete quantifier"),
        (r"a{3,2}", "numbers out of order in {} quantifier"),
        (r"(?=a)*", "Invalid quantifier"),
        (r"(?<=a)+", "Invalid quantifier"),
        (r"\c", "Invalid Unicode escape"),
        (r"\c1", "Invalid Unicode escape"),
        (r"[\c_]", "Invalid Unicode escape"),
        (r"\x4", "Invalid escape"),
        (r"\x{41}", "Invalid escape"),
        (r"\u12", "Invalid Unicode escape"),
        (r"\u{110000}", "Invalid Unicode escape"),
        (r"\-", "Invalid escape"),
        (r"\a", "Invalid escape"),
        (r"\A", "Invalid escape"),
        (r"\z", "Invalid escape"),
        (r"\e", "Invalid escape"),
        (r"\_", "Invalid escape"),
        (r"\ ", "Invalid escape"),
        (r"[\B]", "Invalid escape"),
        (r"[\k]", "Invalid escape"),
        (r"[\1]", "Invalid decimal escape"),
        (r"\00", "Invalid decimal escape"),
        (r"\1", "Invalid escape"),
        (r"(a)\2", "Invalid escape"),
        (r"\k", "Invalid named reference"),
        (r"\k<a", "Invalid capture group name"),
        (r"(?<a>x)\k<b>", "Invalid named capture referenced"),
        (r"(?<a>x)(?<a>y)", "Duplicate capture group name"),
        (r"(?<1a>x)", "Invalid capture group name"),
        (r"(?<a-b>x)", "Invalid capture group name"),
        (r"(?P<a>x)", "Invalid group"),
        (r"(?x)", "Invalid group"),
        (r"(?i)", "Invalid group"),
        (r"(?i-i:a)", "Repeated flag in flag group"),
        (r"(?-:a)", "Invalid flag group"),
        (r"[[:alpha:]]", "Lone quantifier brackets"),
        ("a\\", "\\ at end of pattern"),
    ];
    for (source, reason) in cases {
        assert_eq!(syntax(source), reason, "{source}");
    }
}

#[test]
fn accepts_what_js_u_mode_accepts() {
    for source in [
        r"[\-]",
        r"[\/]",
        r"\/",
        r"\u{10FFFF}",
        r"\u{0000000041}",
        r"a{99999999999999999999}",
        r"(?<\u0061>x)\k<a>",
        r"(?<$_a1>x)",
        r"(?<a>x)|(?<a>y)",
        r"[--a]",
        r"[\d-]",
        r"[(){}|.*+?^$]",
    ] {
        assert!(js_to_rust(source).is_ok(), "{source}");
    }
}

#[test]
fn counts_past_what_rust_can_write_mean_what_they_mean_in_js() {
    assert_eq!(rust("a{99999999999999999999}"), r"[^\x{0}-\x{10FFFF}]");
    assert_eq!(rust("a{1,99999999999999999999}"), "(?:a)+");
    assert_eq!(rust("a{4294967295}"), "(?:a){4294967295}");
    unsupported("(?:a?){99999999999999999999}");
}

#[test]
fn refuses_groups_nested_too_deeply() {
    let deep = format!("{}a{}", "(".repeat(300), ")".repeat(300));
    unsupported(&deep);
    let fine = format!("{}a{}", "(".repeat(200), ")".repeat(200));
    assert!(js_to_rust(&fine).is_ok());
}

#[test]
fn leaves_shared_syntax_alone() {
    assert_eq!(rust(r"(?:a|b)*"), r"(?:(?:a|b))*");
    assert_eq!(rust(r"over(?![a-z-])"), r"over(?![a-z\-])");
    assert_eq!(rust(r"a+?b{2,}c{1,3}?"), r"(?:a)+?(?:b){2,}(?:c){1,3}?");
}

#[test]
fn finds_only_unescaped_end_anchors() {
    assert!(ends_with_anchor(".$"));
    assert!(ends_with_anchor(r"\\$"));
    assert!(!ends_with_anchor(r"\$"));
    assert!(!ends_with_anchor("a"));
}
