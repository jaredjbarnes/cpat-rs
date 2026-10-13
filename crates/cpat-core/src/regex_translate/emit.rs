//! Writes a parsed JS regex in Rust syntax with the same meaning.
//!
//! Every JS-specific meaning is spelled out rather than borrowed from a Rust
//! escape that looks the same: `.`, `\s`, `\d`, `\w` and `\b` all draw their
//! lines elsewhere in Rust. Characters are written as themselves when Rust
//! reads them literally and as `\x{…}` otherwise, so no two emitted pieces
//! can run together into something else.

use std::fmt::Write;

use super::Dialect;
use super::ast::{Assertion, Class, ClassItem, GroupKind, Look, Node, Repeat, Set};
use super::char_sets::{DIGIT, LINE_TERMINATORS, SPACE, WORD};
use super::mode::Mode;

/// Matches no character: an empty JS class, or a lone surrogate (no Rust
/// text holds one).
const NOTHING: &str = r"[^\x{0}-\x{10FFFF}]";
/// Matches any character: `[^]`, or `.` with the `s` flag.
const ANYTHING: &str = "(?s:.)";

pub(super) fn emit(root: &Node, dialect: Dialect) -> String {
    let mut emitter = Emitter {
        dialect,
        out: String::new(),
        open_captures: Vec::new(),
    };
    emitter.node(root, Mode::default());
    emitter.out
}

struct Emitter {
    dialect: Dialect,
    out: String,
    /// Captures being emitted around the current point. A backreference
    /// inside its own group reads nothing in JS.
    open_captures: Vec<usize>,
}

impl Emitter {
    fn node(&mut self, node: &Node, mode: Mode) {
        match node {
            Node::Alternation(alternatives) => {
                for (i, alternative) in alternatives.iter().enumerate() {
                    if i > 0 {
                        self.out.push('|');
                    }
                    self.node(alternative, mode);
                }
            }
            Node::Sequence(terms) => terms.iter().for_each(|term| self.node(term, mode)),
            Node::Char(code) => match char::from_u32(*code) {
                Some(c) => push_char(&mut self.out, c),
                None => self.out.push_str(NOTHING),
            },
            Node::Dot if mode.dot_all => self.out.push_str(ANYTHING),
            Node::Dot => push_ranges_class(&mut self.out, LINE_TERMINATORS, true),
            Node::Set(set) => self.set(set),
            Node::Class(class) => self.class(class),
            Node::Assertion(assertion) => self.assertion(*assertion, mode),
            Node::Group(group) => {
                let inner = match group.kind {
                    GroupKind::Capture(index) => {
                        self.out.push('(');
                        self.open_captures.push(index);
                        mode
                    }
                    GroupKind::NonCapture => {
                        self.out.push_str("(?:");
                        mode
                    }
                    GroupKind::Modifiers(flags) => {
                        let inner = mode.with(flags);
                        // Only `i` is Rust's to apply; `m` and `s` change how
                        // `^`, `$` and `.` are spelled here.
                        self.out
                            .push_str(match (mode.ignore_case, inner.ignore_case) {
                                (false, true) => "(?i:",
                                (true, false) => "(?-i:",
                                _ => "(?:",
                            });
                        inner
                    }
                };
                self.node(&group.body, inner);
                if let GroupKind::Capture(_) = group.kind {
                    self.open_captures.pop();
                }
                self.out.push(')');
            }
            Node::Look(look) => self.look(look, mode),
            Node::Repeat(repeat) => self.repeat(repeat, mode),
            Node::Backreference(reference) => {
                let readable: Vec<usize> = reference
                    .groups
                    .iter()
                    .copied()
                    .filter(|group| !self.open_captures.contains(group))
                    .collect();
                // A group that hasn't matched reads as empty in JS, where
                // fancy-regex would fail: so each group is read only if set.
                for group in &readable {
                    write!(self.out, r"(?({group})\{group}|").expect("writing to a String");
                }
                self.out.push_str(&")".repeat(readable.len()));
            }
        }
    }

    fn set(&mut self, set: &Set) {
        match set {
            Set::Property { negated, name } => push_property(&mut self.out, *negated, name),
            Set::Digit { negated } => push_ranges_class(&mut self.out, DIGIT, *negated),
            Set::Word { negated } => push_ranges_class(&mut self.out, WORD, *negated),
            Set::Space { negated } => push_ranges_class(&mut self.out, SPACE, *negated),
            Set::Surrogates { negated: false } => self.out.push_str(NOTHING),
            Set::Surrogates { negated: true } => self.out.push_str(ANYTHING),
        }
    }

    fn class(&mut self, class: &Class) {
        let mut items = String::new();
        for item in &class.items {
            match item {
                ClassItem::Range(lo, hi) => push_range(&mut items, *lo, *hi),
                // A negated set nests as a class of its own: Rust classes
                // take the union of what they hold.
                ClassItem::Set(Set::Property { negated, name }) => {
                    push_property(&mut items, *negated, name);
                }
                ClassItem::Set(Set::Digit { negated }) => {
                    push_set_item(&mut items, DIGIT, *negated)
                }
                ClassItem::Set(Set::Word { negated }) => push_set_item(&mut items, WORD, *negated),
                ClassItem::Set(Set::Space { negated }) => {
                    push_set_item(&mut items, SPACE, *negated)
                }
                ClassItem::Set(Set::Surrogates { negated: false }) => {}
                ClassItem::Set(Set::Surrogates { negated: true }) => {
                    push_range(&mut items, 0, 0x10FFFF);
                }
            }
        }
        match (items.is_empty(), class.negated) {
            (true, true) => self.out.push_str(ANYTHING),
            (true, false) => self.out.push_str(NOTHING),
            (false, negated) => {
                self.out.push_str(if negated { "[^" } else { "[" });
                self.out.push_str(&items);
                self.out.push(']');
            }
        }
    }

    fn assertion(&mut self, assertion: Assertion, mode: Mode) {
        let out = &mut self.out;
        match (assertion, mode.multiline) {
            // Rust's `^`/`$` without its own `m` flag are start and end of the
            // text, as JS's are without `m`.
            (Assertion::Start, false) => out.push('^'),
            (Assertion::End, false) => out.push('$'),
            // JS's `m` breaks lines at `\r`, U+2028 and U+2029 too.
            (Assertion::Start, true) => {
                out.push_str("(?:^|(?<=");
                push_ranges_class(out, LINE_TERMINATORS, false);
                out.push_str("))");
            }
            (Assertion::End, true) => {
                out.push_str("(?:$|(?=");
                push_ranges_class(out, LINE_TERMINATORS, false);
                out.push_str("))");
            }
            (Assertion::WordBoundary, _) => self.word_boundary(true),
            (Assertion::NotWordBoundary, _) => self.word_boundary(false),
        }
    }

    /// An ASCII word boundary. `regex` switches Unicode off for it, which
    /// `fancy-regex` refuses, so there it is spelled with lookaround. Under
    /// `i` the engine is always `Fancy`: JS's `\b` then also counts `ſ` and
    /// the Kelvin sign as word characters, which `(?i)` adds to the class.
    fn word_boundary(&mut self, boundary: bool) {
        let out = &mut self.out;
        match (self.dialect, boundary) {
            (Dialect::Fast, true) => out.push_str(r"(?-u:\b)"),
            (Dialect::Fast, false) => out.push_str(r"(?-u:\B)"),
            (Dialect::Fancy, _) => {
                let mut word = String::new();
                push_ranges_class(&mut word, WORD, false);
                let (after_word, before_word) = match boundary {
                    true => ("?!", "?="),
                    false => ("?=", "?!"),
                };
                write!(
                    out,
                    "(?:(?<={word})({after_word}{word})|(?<!{word})({before_word}{word}))"
                )
                .expect("writing to a String");
            }
        }
    }

    fn look(&mut self, look: &Look, mode: Mode) {
        self.out.push_str(match (look.behind, look.negated) {
            (false, false) => "(?=",
            (false, true) => "(?!",
            (true, false) => "(?<=",
            (true, true) => "(?<!",
        });
        self.node(&look.body, mode);
        self.out.push(')');
    }

    fn repeat(&mut self, repeat: &Repeat, mode: Mode) {
        self.out.push_str("(?:");
        self.node(&repeat.body, mode);
        self.out.push(')');
        match (repeat.min, repeat.max) {
            (0, None) => self.out.push('*'),
            (1, None) => self.out.push('+'),
            (0, Some(1)) => self.out.push('?'),
            (min, None) => write!(self.out, "{{{min},}}").expect("writing to a String"),
            (min, Some(max)) if min == max => {
                write!(self.out, "{{{min}}}").expect("writing to a String");
            }
            (min, Some(max)) => write!(self.out, "{{{min},{max}}}").expect("writing to a String"),
        }
        if !repeat.greedy {
            self.out.push('?');
        }
    }
}

/// Characters Rust gives a meaning to somewhere (`#`, `&`, `-`, `~` in a
/// class or in verbose mode); escaped, they are literal everywhere.
fn is_rust_meta(c: char) -> bool {
    matches!(
        c,
        '\\' | '.'
            | '+'
            | '*'
            | '?'
            | '('
            | ')'
            | '|'
            | '['
            | ']'
            | '{'
            | '}'
            | '^'
            | '$'
            | '#'
            | '&'
            | '-'
            | '~'
    )
}

fn push_char(out: &mut String, c: char) {
    if is_rust_meta(c) {
        out.push('\\');
        out.push(c);
    } else if c.is_ascii_graphic() || c.is_alphanumeric() {
        out.push(c);
    } else {
        write!(out, "\\x{{{:X}}}", c as u32).expect("writing to a String");
    }
}

/// A class item for an inclusive code-point range. Surrogates are left
/// out: they can't be written in Rust and no Rust text holds one.
fn push_range(out: &mut String, lo: u32, hi: u32) {
    let parts = [(lo, hi.min(0xD7FF)), (lo.max(0xE000), hi)];
    for (lo, hi) in parts.into_iter().filter(|(lo, hi)| lo <= hi) {
        let (Some(first), Some(last)) = (char::from_u32(lo), char::from_u32(hi)) else {
            continue;
        };
        push_char(out, first);
        if last != first {
            out.push('-');
            push_char(out, last);
        }
    }
}

/// A predefined set inside a class: its ranges, or a nested `[^…]`.
fn push_set_item(out: &mut String, ranges: &[(u32, u32)], negated: bool) {
    match negated {
        true => push_ranges_class(out, ranges, true),
        false => ranges.iter().for_each(|&(lo, hi)| push_range(out, lo, hi)),
    }
}

/// `[…]` or `[^…]` over `ranges`.
fn push_ranges_class(out: &mut String, ranges: &[(u32, u32)], negated: bool) {
    out.push_str(if negated { "[^" } else { "[" });
    ranges.iter().for_each(|&(lo, hi)| push_range(out, lo, hi));
    out.push(']');
}

/// `\p{…}`: JS's spellings are ones Rust's loose matching also reads.
fn push_property(out: &mut String, negated: bool, name: &str) {
    write!(out, "\\{}{{{name}}}", if negated { 'P' } else { 'p' }).expect("writing to a String");
}
