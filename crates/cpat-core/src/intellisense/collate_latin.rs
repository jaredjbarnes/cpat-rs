//! Latin letters with a diacritic, as JS's default collation weighs them:
//! the ASCII letter they sort with, the secondary weight of their
//! diacritic, and whether they are upper case.
//!
//! Generated once from Node 24 (ICU, `en-US`) for U+00C0–U+017F. The base
//! letter is the one `localeCompare(…, { sensitivity: "base" })` calls
//! equal. A letter that decomposes to base + combining mark weighs
//! `4 × rank`, the mark's place in the order `"a" + mark` sorts in (1
//! acute, 2 grave, 3 breve, 4 circumflex, 5 caron, 6 ring, 7 diaeresis, 8
//! double acute, 9 tilde, 10 dot above, 11 cedilla, 12 ogonek, 13 macron).
//! A letter with no combining mark that still sorts with its base (a
//! stroke: `ø`, `đ`, `ł`, also `ð`, `ſ`) weighs `4k + 2`, where `k` marked
//! letters of its base sort before it, plus its place among such letters
//! of one base. `ſ` weighs 255, above every diacritic and above `ß`'s
//! second element (`collate::DIGRAPH`), as Node orders them (`ß < ſſ`,
//! `ß̀ < ſś`). `ŀ` is left out: ICU
//! sorts it as `l·`, two primaries.
//! Sorted by `char`, for binary search.

pub(super) const LATIN: &[(char, u8, u8, bool)] = &[
    ('\u{C0}', b'a', 8, true),     // À
    ('\u{C1}', b'a', 4, true),     // Á
    ('\u{C2}', b'a', 16, true),    // Â
    ('\u{C3}', b'a', 36, true),    // Ã
    ('\u{C4}', b'a', 28, true),    // Ä
    ('\u{C5}', b'a', 24, true),    // Å
    ('\u{C7}', b'c', 44, true),    // Ç
    ('\u{C8}', b'e', 8, true),     // È
    ('\u{C9}', b'e', 4, true),     // É
    ('\u{CA}', b'e', 16, true),    // Ê
    ('\u{CB}', b'e', 28, true),    // Ë
    ('\u{CC}', b'i', 8, true),     // Ì
    ('\u{CD}', b'i', 4, true),     // Í
    ('\u{CE}', b'i', 16, true),    // Î
    ('\u{CF}', b'i', 28, true),    // Ï
    ('\u{D0}', b'd', 55, true),    // Ð
    ('\u{D1}', b'n', 36, true),    // Ñ
    ('\u{D2}', b'o', 8, true),     // Ò
    ('\u{D3}', b'o', 4, true),     // Ó
    ('\u{D4}', b'o', 16, true),    // Ô
    ('\u{D5}', b'o', 36, true),    // Õ
    ('\u{D6}', b'o', 28, true),    // Ö
    ('\u{D8}', b'o', 42, true),    // Ø
    ('\u{D9}', b'u', 8, true),     // Ù
    ('\u{DA}', b'u', 4, true),     // Ú
    ('\u{DB}', b'u', 16, true),    // Û
    ('\u{DC}', b'u', 28, true),    // Ü
    ('\u{DD}', b'y', 4, true),     // Ý
    ('\u{E0}', b'a', 8, false),    // à
    ('\u{E1}', b'a', 4, false),    // á
    ('\u{E2}', b'a', 16, false),   // â
    ('\u{E3}', b'a', 36, false),   // ã
    ('\u{E4}', b'a', 28, false),   // ä
    ('\u{E5}', b'a', 24, false),   // å
    ('\u{E7}', b'c', 44, false),   // ç
    ('\u{E8}', b'e', 8, false),    // è
    ('\u{E9}', b'e', 4, false),    // é
    ('\u{EA}', b'e', 16, false),   // ê
    ('\u{EB}', b'e', 28, false),   // ë
    ('\u{EC}', b'i', 8, false),    // ì
    ('\u{ED}', b'i', 4, false),    // í
    ('\u{EE}', b'i', 16, false),   // î
    ('\u{EF}', b'i', 28, false),   // ï
    ('\u{F0}', b'd', 55, false),   // ð
    ('\u{F1}', b'n', 36, false),   // ñ
    ('\u{F2}', b'o', 8, false),    // ò
    ('\u{F3}', b'o', 4, false),    // ó
    ('\u{F4}', b'o', 16, false),   // ô
    ('\u{F5}', b'o', 36, false),   // õ
    ('\u{F6}', b'o', 28, false),   // ö
    ('\u{F8}', b'o', 42, false),   // ø
    ('\u{F9}', b'u', 8, false),    // ù
    ('\u{FA}', b'u', 4, false),    // ú
    ('\u{FB}', b'u', 16, false),   // û
    ('\u{FC}', b'u', 28, false),   // ü
    ('\u{FD}', b'y', 4, false),    // ý
    ('\u{FF}', b'y', 28, false),   // ÿ
    ('\u{100}', b'a', 52, true),   // Ā
    ('\u{101}', b'a', 52, false),  // ā
    ('\u{102}', b'a', 12, true),   // Ă
    ('\u{103}', b'a', 12, false),  // ă
    ('\u{104}', b'a', 48, true),   // Ą
    ('\u{105}', b'a', 48, false),  // ą
    ('\u{106}', b'c', 4, true),    // Ć
    ('\u{107}', b'c', 4, false),   // ć
    ('\u{108}', b'c', 16, true),   // Ĉ
    ('\u{109}', b'c', 16, false),  // ĉ
    ('\u{10A}', b'c', 40, true),   // Ċ
    ('\u{10B}', b'c', 40, false),  // ċ
    ('\u{10C}', b'c', 20, true),   // Č
    ('\u{10D}', b'c', 20, false),  // č
    ('\u{10E}', b'd', 20, true),   // Ď
    ('\u{10F}', b'd', 20, false),  // ď
    ('\u{110}', b'd', 54, true),   // Đ
    ('\u{111}', b'd', 54, false),  // đ
    ('\u{112}', b'e', 52, true),   // Ē
    ('\u{113}', b'e', 52, false),  // ē
    ('\u{114}', b'e', 12, true),   // Ĕ
    ('\u{115}', b'e', 12, false),  // ĕ
    ('\u{116}', b'e', 40, true),   // Ė
    ('\u{117}', b'e', 40, false),  // ė
    ('\u{118}', b'e', 48, true),   // Ę
    ('\u{119}', b'e', 48, false),  // ę
    ('\u{11A}', b'e', 20, true),   // Ě
    ('\u{11B}', b'e', 20, false),  // ě
    ('\u{11C}', b'g', 16, true),   // Ĝ
    ('\u{11D}', b'g', 16, false),  // ĝ
    ('\u{11E}', b'g', 12, true),   // Ğ
    ('\u{11F}', b'g', 12, false),  // ğ
    ('\u{120}', b'g', 40, true),   // Ġ
    ('\u{121}', b'g', 40, false),  // ġ
    ('\u{122}', b'g', 44, true),   // Ģ
    ('\u{123}', b'g', 44, false),  // ģ
    ('\u{124}', b'h', 16, true),   // Ĥ
    ('\u{125}', b'h', 16, false),  // ĥ
    ('\u{126}', b'h', 54, true),   // Ħ
    ('\u{127}', b'h', 54, false),  // ħ
    ('\u{128}', b'i', 36, true),   // Ĩ
    ('\u{129}', b'i', 36, false),  // ĩ
    ('\u{12A}', b'i', 52, true),   // Ī
    ('\u{12B}', b'i', 52, false),  // ī
    ('\u{12C}', b'i', 12, true),   // Ĭ
    ('\u{12D}', b'i', 12, false),  // ĭ
    ('\u{12E}', b'i', 48, true),   // Į
    ('\u{12F}', b'i', 48, false),  // į
    ('\u{130}', b'i', 40, true),   // İ
    ('\u{134}', b'j', 16, true),   // Ĵ
    ('\u{135}', b'j', 16, false),  // ĵ
    ('\u{136}', b'k', 44, true),   // Ķ
    ('\u{137}', b'k', 44, false),  // ķ
    ('\u{139}', b'l', 4, true),    // Ĺ
    ('\u{13A}', b'l', 4, false),   // ĺ
    ('\u{13B}', b'l', 44, true),   // Ļ
    ('\u{13C}', b'l', 44, false),  // ļ
    ('\u{13D}', b'l', 20, true),   // Ľ
    ('\u{13E}', b'l', 20, false),  // ľ
    ('\u{141}', b'l', 54, true),   // Ł
    ('\u{142}', b'l', 54, false),  // ł
    ('\u{143}', b'n', 4, true),    // Ń
    ('\u{144}', b'n', 4, false),   // ń
    ('\u{145}', b'n', 44, true),   // Ņ
    ('\u{146}', b'n', 44, false),  // ņ
    ('\u{147}', b'n', 20, true),   // Ň
    ('\u{148}', b'n', 20, false),  // ň
    ('\u{14C}', b'o', 52, true),   // Ō
    ('\u{14D}', b'o', 52, false),  // ō
    ('\u{14E}', b'o', 12, true),   // Ŏ
    ('\u{14F}', b'o', 12, false),  // ŏ
    ('\u{150}', b'o', 32, true),   // Ő
    ('\u{151}', b'o', 32, false),  // ő
    ('\u{154}', b'r', 4, true),    // Ŕ
    ('\u{155}', b'r', 4, false),   // ŕ
    ('\u{156}', b'r', 44, true),   // Ŗ
    ('\u{157}', b'r', 44, false),  // ŗ
    ('\u{158}', b'r', 20, true),   // Ř
    ('\u{159}', b'r', 20, false),  // ř
    ('\u{15A}', b's', 4, true),    // Ś
    ('\u{15B}', b's', 4, false),   // ś
    ('\u{15C}', b's', 16, true),   // Ŝ
    ('\u{15D}', b's', 16, false),  // ŝ
    ('\u{15E}', b's', 44, true),   // Ş
    ('\u{15F}', b's', 44, false),  // ş
    ('\u{160}', b's', 20, true),   // Š
    ('\u{161}', b's', 20, false),  // š
    ('\u{162}', b't', 44, true),   // Ţ
    ('\u{163}', b't', 44, false),  // ţ
    ('\u{164}', b't', 20, true),   // Ť
    ('\u{165}', b't', 20, false),  // ť
    ('\u{168}', b'u', 36, true),   // Ũ
    ('\u{169}', b'u', 36, false),  // ũ
    ('\u{16A}', b'u', 52, true),   // Ū
    ('\u{16B}', b'u', 52, false),  // ū
    ('\u{16C}', b'u', 12, true),   // Ŭ
    ('\u{16D}', b'u', 12, false),  // ŭ
    ('\u{16E}', b'u', 24, true),   // Ů
    ('\u{16F}', b'u', 24, false),  // ů
    ('\u{170}', b'u', 32, true),   // Ű
    ('\u{171}', b'u', 32, false),  // ű
    ('\u{172}', b'u', 48, true),   // Ų
    ('\u{173}', b'u', 48, false),  // ų
    ('\u{174}', b'w', 16, true),   // Ŵ
    ('\u{175}', b'w', 16, false),  // ŵ
    ('\u{176}', b'y', 16, true),   // Ŷ
    ('\u{177}', b'y', 16, false),  // ŷ
    ('\u{178}', b'y', 28, true),   // Ÿ
    ('\u{179}', b'z', 4, true),    // Ź
    ('\u{17A}', b'z', 4, false),   // ź
    ('\u{17B}', b'z', 40, true),   // Ż
    ('\u{17C}', b'z', 40, false),  // ż
    ('\u{17D}', b'z', 20, true),   // Ž
    ('\u{17E}', b'z', 20, false),  // ž
    ('\u{17F}', b's', 255, false), // ſ
];
