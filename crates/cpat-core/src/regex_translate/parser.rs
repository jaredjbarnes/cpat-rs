//! Reads a regex source the way JS does with the `u` flag (ES2025, without
//! `v`), and refuses what it refuses. Rust's parsers are more lenient
//! (`\pL`, `[\d-z]`, `a{,3}`, `\-`, `\x{41}`, possessive `a*+`…), so a
//! source Rust would happily compile must first pass here.

use std::sync::LazyLock;

use super::ast::{
    Assertion, Backreference, Class, ClassItem, Flags, Group, GroupKind, Look, Node, Repeat, Set,
};
use super::error::TranslateError::{self, Syntax, Unsupported};
use super::js_property_names::{BINARY_PROPERTIES, GENERAL_CATEGORY_VALUES, SCRIPT_VALUES};

/// How deeply groups may nest. JS has no limit, but every pass here is
/// recursive, and a source is text a grammar author (or its user) wrote.
pub(crate) const MAX_DEPTH: usize = 256;

/// The largest repetition count Rust can write. JS allows up to 2^53 - 1.
const MAX_COUNT: u64 = u32::MAX as u64;

/// The parsed source.
pub(super) fn parse(source: &str) -> Result<Node, TranslateError> {
    let chars: Vec<char> = source.chars().collect();
    let capture_names = scan_captures(&chars);
    let mut parser = Parser {
        chars,
        pos: 0,
        capture_names,
        next_capture: 1,
        events: 0,
        depth: 0,
        path: Vec::new(),
        next_disjunction: 0,
        named: Vec::new(),
    };

    let root = parser.disjunction()?;
    match parser.peek() {
        None => Ok(root),
        Some(_) => Err(Syntax("Unmatched ')'")),
    }
}

type Parse<T> = Result<T, TranslateError>;

struct Parser {
    chars: Vec<char>,
    pos: usize,
    /// Every capture's name (`None` if unnamed), in order, read ahead:
    /// JS lets a backreference come before its group.
    capture_names: Vec<Option<String>>,
    next_capture: usize,
    /// Counts groups closing and backreferences read, so the backreference
    /// check can tell which came first.
    events: usize,
    depth: usize,
    /// The (disjunction, alternative) pairs leading to the current term.
    /// Two groups may share a name only where they can't both match: in
    /// different alternatives of one disjunction.
    path: Vec<(usize, usize)>,
    next_disjunction: usize,
    named: Vec<(String, Vec<(usize, usize)>)>,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += 1;
        Some(c)
    }

    fn eat(&mut self, c: char) -> bool {
        let found = self.peek() == Some(c);
        if found {
            self.pos += 1;
        }
        found
    }

    fn event(&mut self) -> usize {
        self.events += 1;
        self.events
    }

    fn disjunction(&mut self) -> Parse<Node> {
        let id = self.next_disjunction;
        self.next_disjunction += 1;

        let mut alternatives = Vec::new();
        loop {
            self.path.push((id, alternatives.len()));
            let alternative = self.alternative();
            self.path.pop();
            alternatives.push(alternative?);
            if !self.eat('|') {
                break;
            }
        }

        Ok(match alternatives.len() {
            1 => alternatives.pop().expect("one alternative"),
            _ => Node::Alternation(alternatives),
        })
    }

    fn alternative(&mut self) -> Parse<Node> {
        let mut terms = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            terms.push(self.term()?);
        }
        Ok(Node::Sequence(terms))
    }

    fn term(&mut self) -> Parse<Node> {
        let (atom, quantifiable) = self.atom()?;
        let Some((min, max)) = self.quantifier()? else {
            return Ok(atom);
        };
        let greedy = !self.eat('?');

        match (quantifiable, &atom) {
            (true, _) if min > MAX_COUNT && atom.nullable() => Err(Unsupported(
                "a repetition count over 4294967295 of something that can match nothing",
            )),
            // No text is long enough, as JS would find out by trying.
            (true, _) if min > MAX_COUNT => Ok(Node::Class(Class {
                negated: false,
                items: Vec::new(),
            })),
            (true, _) => Ok(Node::Repeat(Repeat {
                body: Box::new(atom),
                min,
                // No text has that many characters: the same as no limit.
                max: max.filter(|&max| max <= MAX_COUNT),
                greedy,
            })),
            (false, Node::Look(_)) => Err(Syntax("Invalid quantifier")),
            (false, _) => Err(Syntax("Nothing to repeat")),
        }
    }

    /// The next atom, and whether JS (`u` flag) lets a quantifier follow it:
    /// not after an assertion or a lookaround.
    fn atom(&mut self) -> Parse<(Node, bool)> {
        let c = self.bump().expect("term() is called before the end");
        let atom = match c {
            '^' => (Node::Assertion(Assertion::Start), false),
            '$' => (Node::Assertion(Assertion::End), false),
            '\\' if self.eat('b') => (Node::Assertion(Assertion::WordBoundary), false),
            '\\' if self.eat('B') => (Node::Assertion(Assertion::NotWordBoundary), false),
            '\\' => (self.atom_escape()?, true),
            '(' => self.group()?,
            '.' => (Node::Dot, true),
            '[' => (Node::Class(self.class()?), true),
            '*' | '+' | '?' => return Err(Syntax("Nothing to repeat")),
            '{' => {
                self.pos -= 1;
                return Err(match self.quantifier() {
                    Ok(Some(_)) => Syntax("Nothing to repeat"),
                    _ => Syntax("Lone quantifier brackets"),
                });
            }
            '}' | ']' => return Err(Syntax("Lone quantifier brackets")),
            c => (Node::Char(c as u32), true),
        };
        Ok(atom)
    }

    /// `*`, `+`, `?` or `{n}`, `{n,}`, `{n,m}` as (min, max). A `{` that
    /// starts none of those is an error with the `u` flag.
    fn quantifier(&mut self) -> Parse<Option<(u64, Option<u64>)>> {
        let bounds = match self.peek() {
            Some('*') => (0, None),
            Some('+') => (1, None),
            Some('?') => (0, Some(1)),
            Some('{') => {
                self.pos += 1;
                let min = self.number().ok_or(Syntax("Incomplete quantifier"))?;
                let max = if self.eat(',') {
                    match self.peek() {
                        Some('}') => None,
                        _ => Some(self.number().ok_or(Syntax("Incomplete quantifier"))?),
                    }
                } else {
                    Some(min)
                };
                if self.peek() != Some('}') {
                    return Err(Syntax("Incomplete quantifier"));
                }
                if max.is_some_and(|max| max < min) {
                    return Err(Syntax("numbers out of order in {} quantifier"));
                }
                (min, max)
            }
            _ => return Ok(None),
        };
        self.pos += 1;
        Ok(Some(bounds))
    }

    /// Decimal digits, saturating as JS clamps a huge count.
    fn number(&mut self) -> Option<u64> {
        let start = self.pos;
        let mut value: u64 = 0;
        while let Some(digit) = self.peek().and_then(|c| c.to_digit(10)) {
            value = value.saturating_mul(10).saturating_add(digit.into());
            self.pos += 1;
        }
        (self.pos > start).then_some(value)
    }

    /// After `(`: a group of any kind, and whether it may be quantified.
    fn group(&mut self) -> Parse<(Node, bool)> {
        if self.depth >= MAX_DEPTH {
            return Err(Unsupported("groups nested more than 256 deep"));
        }

        let kind = if self.eat('?') {
            match self.bump() {
                Some(':') => Opening::Group(GroupKind::NonCapture),
                Some('=') => Opening::Look {
                    behind: false,
                    negated: false,
                },
                Some('!') => Opening::Look {
                    behind: false,
                    negated: true,
                },
                Some('<') if self.eat('=') => Opening::Look {
                    behind: true,
                    negated: false,
                },
                Some('<') if self.eat('!') => Opening::Look {
                    behind: true,
                    negated: true,
                },
                Some('<') => {
                    let name = self.group_name()?;
                    self.declare(name)?;
                    Opening::Group(self.capture())
                }
                Some('i' | 'm' | 's' | '-') => {
                    self.pos -= 1;
                    Opening::Group(GroupKind::Modifiers(self.modifiers()?))
                }
                _ => return Err(Syntax("Invalid group")),
            }
        } else {
            Opening::Group(self.capture())
        };

        self.depth += 1;
        let body = self.disjunction();
        self.depth -= 1;
        let body = Box::new(body?);
        if !self.eat(')') {
            return Err(Syntax("Unterminated group"));
        }

        Ok(match kind {
            Opening::Group(kind) => {
                let end = self.event();
                (Node::Group(Group { kind, body, end }), true)
            }
            Opening::Look { behind, negated } => (
                Node::Look(Look {
                    behind,
                    negated,
                    body,
                }),
                false,
            ),
        })
    }

    fn capture(&mut self) -> GroupKind {
        let index = self.next_capture;
        self.next_capture += 1;
        GroupKind::Capture(index)
    }

    /// Records a group name, refusing it where another group of that name
    /// could also match.
    fn declare(&mut self, name: String) -> Parse<()> {
        let clash = self
            .named
            .iter()
            .any(|(other, path)| *other == name && can_both_match(path, &self.path));
        if clash {
            return Err(Syntax("Duplicate capture group name"));
        }
        self.named.push((name, self.path.clone()));
        Ok(())
    }

    /// `ims-ims:` after `(?`.
    fn modifiers(&mut self) -> Parse<Flags> {
        let mut flags = Flags::default();
        let mut adding = true;
        let mut any = false;
        loop {
            let slot = match self.bump() {
                Some(':') => break,
                Some('-') if adding => {
                    adding = false;
                    continue;
                }
                Some('i') => &mut flags.ignore_case,
                Some('m') => &mut flags.multiline,
                Some('s') => &mut flags.dot_all,
                _ => return Err(Syntax("Invalid group")),
            };
            if slot.is_some() {
                return Err(Syntax("Repeated flag in flag group"));
            }
            *slot = Some(adding);
            any = true;
        }
        if !any {
            return Err(Syntax("Invalid flag group"));
        }
        Ok(flags)
    }

    /// A group name up to its `>`.
    fn group_name(&mut self) -> Parse<String> {
        read_group_name(&self.chars, &mut self.pos).ok_or(Syntax("Invalid capture group name"))
    }

    /// After a `\` outside a class (`\b` and `\B` are taken already).
    fn atom_escape(&mut self) -> Parse<Node> {
        let Some(c) = self.bump() else {
            return Err(Syntax("\\ at end of pattern"));
        };
        match c {
            '1'..='9' => {
                self.pos -= 1;
                let number = self.number().expect("a digit is next");
                if number > self.capture_names.len() as u64 {
                    return Err(Syntax("Invalid escape"));
                }
                Ok(self.backreference(vec![number as usize]))
            }
            'k' => {
                if !self.eat('<') {
                    return Err(Syntax("Invalid named reference"));
                }
                let name = self.group_name()?;
                let groups: Vec<usize> = self
                    .capture_names
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| n.as_deref() == Some(name.as_str()))
                    .map(|(i, _)| i + 1)
                    .collect();
                if groups.is_empty() {
                    return Err(Syntax("Invalid named capture referenced"));
                }
                Ok(self.backreference(groups))
            }
            _ => match self.set_escape(c, false)? {
                Some(set) => Ok(Node::Set(set)),
                None => Ok(Node::Char(self.character_escape(c, false)?)),
            },
        }
    }

    fn backreference(&mut self, groups: Vec<usize>) -> Node {
        let at = self.event();
        Node::Backreference(Backreference { groups, at })
    }

    /// `\d \D \w \W \s \S \p{…} \P{…}`, or `None` for any other escape.
    fn set_escape(&mut self, c: char, in_class: bool) -> Parse<Option<Set>> {
        Ok(Some(match c {
            'd' => Set::Digit { negated: false },
            'D' => Set::Digit { negated: true },
            'w' => Set::Word { negated: false },
            'W' => Set::Word { negated: true },
            's' => Set::Space { negated: false },
            'S' => Set::Space { negated: true },
            'p' | 'P' => {
                let negated = c == 'P';
                let name = self.property_name(in_class)?;
                match is_surrogate_category(&name) {
                    true => Set::Surrogates { negated },
                    false => Set::Property { negated, name },
                }
            }
            _ => return Ok(None),
        }))
    }

    /// `{name}` or `{name=value}` after `\p`, spelled exactly as JS knows it.
    fn property_name(&mut self, in_class: bool) -> Parse<String> {
        let invalid = Syntax(match in_class {
            true => "Invalid property name in character class",
            false => "Invalid property name",
        });
        if !self.eat('{') {
            return Err(invalid);
        }
        let start = self.pos;
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '=')
        {
            self.pos += 1;
        }
        let name: String = self.chars[start..self.pos].iter().collect();
        if !self.eat('}') || !is_js_property(&name) {
            return Err(invalid);
        }
        if lacks_in_rust(&name) {
            return Err(Unsupported(
                "Rust's Unicode tables lack this property (Changes_When_NFKC_Casefolded, Script=Unknown)",
            ));
        }
        Ok(name)
    }

    /// A character escape (after `\`), the same in and out of a class except
    /// for `-`, which only a class may escape.
    fn character_escape(&mut self, c: char, in_class: bool) -> Parse<u32> {
        Ok(match c {
            'f' => 0x0C,
            'n' => 0x0A,
            'r' => 0x0D,
            't' => 0x09,
            'v' => 0x0B,
            'c' => match self.peek() {
                Some(letter) if letter.is_ascii_alphabetic() => {
                    self.pos += 1;
                    letter as u32 % 32
                }
                _ => return Err(Syntax("Invalid Unicode escape")),
            },
            '0' if self.peek().is_some_and(|c| c.is_ascii_digit()) => {
                return Err(Syntax("Invalid decimal escape"));
            }
            '0' => 0,
            '1'..='9' => return Err(Syntax("Invalid decimal escape")),
            'x' => {
                let value = hex(&self.chars, self.pos, 2).ok_or(Syntax("Invalid escape"))?;
                self.pos += 2;
                value
            }
            'u' => unicode_escape(&self.chars, &mut self.pos)
                .ok_or(Syntax("Invalid Unicode escape"))?,
            '^' | '$' | '\\' | '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|'
            | '/' => c as u32,
            '-' if in_class => c as u32,
            _ => return Err(Syntax("Invalid escape")),
        })
    }

    /// After `[`.
    fn class(&mut self) -> Parse<Class> {
        let negated = self.eat('^');
        let mut items = Vec::new();
        loop {
            match self.peek() {
                None => return Err(Syntax("Unterminated character class")),
                Some(']') => {
                    self.pos += 1;
                    break;
                }
                Some(_) => {}
            }
            let first = self.class_atom()?;
            let is_range = self.peek() == Some('-') && !matches!(self.peek_at(1), None | Some(']'));
            if !is_range {
                items.push(first);
                continue;
            }
            self.pos += 1;
            let last = self.class_atom()?;
            match (first, last) {
                (ClassItem::Range(lo, _), ClassItem::Range(hi, _)) if lo > hi => {
                    return Err(Syntax("Range out of order in character class"));
                }
                (ClassItem::Range(lo, _), ClassItem::Range(hi, _)) => {
                    items.push(ClassItem::Range(lo, hi));
                }
                // With the `u` flag a set can't be a range's end: `[\d-z]`.
                _ => return Err(Syntax("Invalid character class")),
            }
        }
        Ok(Class { negated, items })
    }

    fn class_atom(&mut self) -> Parse<ClassItem> {
        let c = self.bump().expect("class() checked for the end");
        if c != '\\' {
            return Ok(ClassItem::Range(c as u32, c as u32));
        }
        let Some(escaped) = self.bump() else {
            return Err(Syntax("\\ at end of pattern"));
        };
        if escaped == 'b' {
            return Ok(ClassItem::Range(0x08, 0x08));
        }
        if let Some(set) = self.set_escape(escaped, true)? {
            return Ok(ClassItem::Set(set));
        }
        let code = self.character_escape(escaped, true)?;
        Ok(ClassItem::Range(code, code))
    }
}

enum Opening {
    Group(GroupKind),
    Look { behind: bool, negated: bool },
}

/// Whether two groups at these paths could both take part in one match:
/// unless they sit in different alternatives of the same disjunction.
fn can_both_match(a: &[(usize, usize)], b: &[(usize, usize)]) -> bool {
    match a.iter().zip(b).find(|(x, y)| x != y) {
        Some((x, y)) => x.0 != y.0,
        None => true,
    }
}

fn is_js_property(name: &str) -> bool {
    let known = |table: &[&str], value: &str| table.binary_search(&value).is_ok();
    match name.split_once('=') {
        Some(("General_Category" | "gc", value)) => known(GENERAL_CATEGORY_VALUES, value),
        Some(("Script" | "sc" | "Script_Extensions" | "scx", value)) => known(SCRIPT_VALUES, value),
        Some(_) => false,
        None => known(GENERAL_CATEGORY_VALUES, name) || known(BINARY_PROPERTIES, name),
    }
}

/// The General_Category value for lone surrogates, however JS spells it.
fn is_surrogate_category(name: &str) -> bool {
    let value = name
        .strip_prefix("gc=")
        .or(name.strip_prefix("General_Category="));
    matches!(value.unwrap_or(name), "Cs" | "Surrogate")
}

/// JS property names regex-syntax (Unicode 16) doesn't know. `Unknown`
/// and `Zzzz` are only ever script values.
fn lacks_in_rust(name: &str) -> bool {
    let value = name.split_once('=').map(|(_, value)| value);
    matches!(name, "CWKCF" | "Changes_When_NFKC_Casefolded")
        || matches!(value, Some("Unknown" | "Zzzz"))
}

/// `count` hex digits at `at`.
fn hex(chars: &[char], at: usize, count: usize) -> Option<u32> {
    let digits = chars.get(at..at + count)?;
    digits
        .iter()
        .try_fold(0u32, |value, c| Some(value * 16 + c.to_digit(16)?))
}

/// After `\u`: `{hex}` up to U+10FFFF, or four hex digits, with an escaped
/// surrogate pair read as the one code point it spells (as the `u` flag does).
fn unicode_escape(chars: &[char], pos: &mut usize) -> Option<u32> {
    if chars.get(*pos) == Some(&'{') {
        let close = *pos + 1 + chars[*pos + 1..].iter().position(|&c| c == '}')?;
        let digits = &chars[*pos + 1..close];
        if digits.is_empty() {
            return None;
        }
        let value = digits.iter().try_fold(0u32, |value, c| {
            let value = value.checked_mul(16)? + c.to_digit(16)?;
            (value <= 0x10FFFF).then_some(value)
        })?;
        *pos = close + 1;
        return Some(value);
    }

    let lead = hex(chars, *pos, 4)?;
    *pos += 4;
    let is_pair = (0xD800..=0xDBFF).contains(&lead)
        && chars.get(*pos..*pos + 2) == Some(&['\\', 'u'])
        && hex(chars, *pos + 2, 4).is_some_and(|t| (0xDC00..=0xDFFF).contains(&t));
    if !is_pair {
        return Some(lead);
    }
    let trail = hex(chars, *pos + 2, 4)?;
    *pos += 6;
    Some(0x10000 + ((lead - 0xD800) << 10) + (trail - 0xDC00))
}

static ID_START: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^[\p{ID_Start}$_]$").expect("valid"));
static ID_CONTINUE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^[\p{ID_Continue}$\x{200C}\x{200D}]$").expect("valid"));

/// A JS identifier (with `\u` escapes) up to `>`, which it consumes.
fn read_group_name(chars: &[char], pos: &mut usize) -> Option<String> {
    let mut name = String::new();
    loop {
        let c = match *chars.get(*pos)? {
            '>' => {
                *pos += 1;
                break;
            }
            '\\' => {
                if chars.get(*pos + 1) != Some(&'u') {
                    return None;
                }
                *pos += 2;
                char::from_u32(unicode_escape(chars, pos)?)?
            }
            c => {
                *pos += 1;
                c
            }
        };
        let valid = match name.is_empty() {
            true => &ID_START,
            false => &ID_CONTINUE,
        };
        if !valid.is_match(c.encode_utf8(&mut [0; 4])) {
            return None;
        }
        name.push(c);
    }
    (!name.is_empty()).then_some(name)
}

/// Every capture's name (or `None`), in order, without parsing: a
/// backreference may come before the group it names.
fn scan_captures(chars: &[char]) -> Vec<Option<String>> {
    let mut captures = Vec::new();
    let mut in_class = false;
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 1,
            '[' if !in_class => in_class = true,
            ']' if in_class => in_class = false,
            '(' if !in_class => match (chars.get(i + 1), chars.get(i + 2)) {
                (Some('?'), Some('<')) if !matches!(chars.get(i + 3), Some('=' | '!')) => {
                    let mut at = i + 3;
                    captures.push(read_group_name(chars, &mut at));
                }
                (Some('?'), _) => {}
                _ => captures.push(None),
            },
            _ => {}
        }
        i += 1;
    }
    captures
}
