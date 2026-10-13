//! JS's fixed character sets as inclusive code-point ranges. Rust's own
//! `\d`, `\w`, `\s` and `.` are Unicode-wide and draw the lines elsewhere,
//! so they are never emitted.

/// `\d`: ASCII digits only.
pub(super) const DIGIT: &[(u32, u32)] = &[(0x30, 0x39)];

/// `\w` (without `i`): ASCII letters, digits and `_`.
pub(super) const WORD: &[(u32, u32)] = &[(0x30, 0x39), (0x41, 0x5A), (0x5F, 0x5F), (0x61, 0x7A)];

/// `\s`: JS's WhiteSpace and LineTerminator. Unlike Unicode's White_Space
/// (Rust's `\s`) it has U+FEFF and lacks U+0085.
pub(super) const SPACE: &[(u32, u32)] = &[
    (0x09, 0x0D), // \t \n \v \f \r
    (0x20, 0x20),
    (0xA0, 0xA0),
    (0x1680, 0x1680),
    (0x2000, 0x200A),
    (0x2028, 0x2029),
    (0x202F, 0x202F),
    (0x205F, 0x205F),
    (0x3000, 0x3000),
    (0xFEFF, 0xFEFF),
];

/// What `.` (without `s`) refuses and where `^`/`$` (with `m`) hold.
pub(super) const LINE_TERMINATORS: &[(u32, u32)] = &[(0x0A, 0x0A), (0x0D, 0x0D), (0x2028, 0x2029)];
