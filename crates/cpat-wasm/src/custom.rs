//! Custom patterns written in JS (DESIGN §9, §22): `(text, pos) => end |
//! null`, positions in UTF-16 offsets, one boundary call per attempt.
//!
//! The text crosses once per parse, not once per attempt: wasm hands it to
//! the host when a parse it hasn't seen asks (keyed on `Cursor::id`), and
//! each attempt then sends only a position. Decoding the whole text on every
//! attempt would make a long input quadratic.

use std::cell::RefCell;
use std::rc::Rc;

use cpat_core::{Cursor, Custom, CustomParse, Meta, NodeId, NodeKind};

use crate::host::{CustomHost, message_of};

/// What the host answers for "no match" (`js/patterns.mjs`): a position
/// can't be negative.
const NO_MATCH: f64 = -1.0;

/// The pattern for `Pattern.custom`.
pub(crate) fn custom(name: &str, host: CustomHost) -> Custom {
    let parser = JsCustom {
        host: Rc::new(host),
        seen: Rc::new(RefCell::new(None)),
    };
    Custom::new(name, Box::new(parser))
}

/// The user's function behind a host. Clones (one per recursion level,
/// DESIGN §1) share the host and so what it was last told.
#[derive(Clone)]
struct JsCustom {
    host: Rc<CustomHost>,
    seen: Rc<RefCell<Option<Seen>>>,
}

/// The parse the host has the text of, and that text's offset maps.
struct Seen {
    cursor: u64,
    map: OffsetMap,
}

impl CustomParse for JsCustom {
    fn parse(&mut self, meta: Meta, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        let first = cursor.pos();
        let answer = self.ask(cursor, first);
        match answer {
            Ok(None) => {
                cursor.record_error(first, first, meta);
                None
            }
            Ok(Some(end)) => {
                // As a regex does: an end inside a grapheme takes the whole
                // grapheme, so the node's value and range agree.
                let end = cursor.snap_forward(end);
                if end == first {
                    // Nothing consumed: the `Custom` contract's zero-width
                    // success.
                    return None;
                }
                cursor.resolve_error();
                let node = cursor.leaf(NodeKind::Custom, meta.name, first, end);
                cursor.record_match(meta, first, end);
                cursor.set_pos(end);
                Some(node)
            }
            Err(reason) => {
                cursor.halt_custom(meta, reason);
                None
            }
        }
    }

    fn clone_box(&self) -> Box<dyn CustomParse> {
        Box::new(self.clone())
    }
}

impl JsCustom {
    /// The host's answer at byte `pos`: `None` for no match, else the end
    /// as a byte offset. `Err` is why the parse must stop.
    fn ask(&self, cursor: &Cursor<'_>, pos: usize) -> Result<Option<usize>, String> {
        let mut seen = self.seen.borrow_mut();
        if seen.as_ref().is_none_or(|seen| seen.cursor != cursor.id()) {
            let text = cursor.text();
            self.host.set_text(text);
            *seen = Some(Seen {
                cursor: cursor.id(),
                map: OffsetMap::new(text),
            });
        }
        let map = &seen.as_ref().expect("set above").map;
        let from = map.unit_of(pos);
        // Let go before calling out: the function may parse again (another
        // pattern, or this one through a copy), which asks this cell.
        let map = map.clone();
        drop(seen);

        let end = self
            .host
            .match_at(from)
            .map_err(|thrown| message_of(&thrown))?;
        if end == NO_MATCH {
            return Ok(None);
        }
        // The host checked the answer against the text; this only keeps a
        // host that didn't from reaching past the text.
        map.byte_of(end, from).map(Some).ok_or_else(|| {
            format!(
                "answered {end} at {from}; return null, or an end from {from} to the text's length"
            )
        })
    }
}

/// UTF-16 offsets to bytes and back, for one text. ASCII needs no table.
#[derive(Clone)]
enum OffsetMap {
    Ascii(usize),
    Tables {
        unit_of_byte: Rc<[u32]>,
        byte_of_unit: Rc<[u32]>,
    },
}

impl OffsetMap {
    fn new(text: &str) -> OffsetMap {
        if text.is_ascii() {
            return OffsetMap::Ascii(text.len());
        }
        let mut unit_of_byte = vec![0u32; text.len() + 1];
        let mut byte_of_unit = Vec::with_capacity(text.len() + 1);
        let mut units = 0u32;
        for (byte, ch) in text.char_indices() {
            unit_of_byte[byte..byte + ch.len_utf8()].fill(units);
            // A unit inside a surrogate pair maps to no byte (see `byte_of`).
            byte_of_unit.push(byte as u32);
            if ch.len_utf16() == 2 {
                byte_of_unit.push(u32::MAX);
            }
            units += ch.len_utf16() as u32;
        }
        unit_of_byte[text.len()] = units;
        byte_of_unit.push(text.len() as u32);
        OffsetMap::Tables {
            unit_of_byte: unit_of_byte.into(),
            byte_of_unit: byte_of_unit.into(),
        }
    }

    fn unit_of(&self, byte: usize) -> u32 {
        match self {
            OffsetMap::Ascii(_) => byte as u32,
            OffsetMap::Tables { unit_of_byte, .. } => unit_of_byte[byte],
        }
    }

    /// The byte where UTF-16 offset `unit` is, when it is a whole number
    /// from `from` to the text's length and not inside a surrogate pair.
    fn byte_of(&self, unit: f64, from: u32) -> Option<usize> {
        if unit.fract() != 0.0 || unit < f64::from(from) {
            return None;
        }
        match self {
            OffsetMap::Ascii(len) => (unit <= *len as f64).then_some(unit as usize),
            OffsetMap::Tables { byte_of_unit, .. } => {
                let byte = *byte_of_unit.get(unit as usize)?;
                (byte != u32::MAX).then_some(byte as usize)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::OffsetMap;

    #[test]
    fn maps_both_ways_around_a_surrogate_pair() {
        // "é" (2 bytes, 1 unit), "😀" (4 bytes, 2 units), "a"
        let map = OffsetMap::new("é😀a");
        assert_eq!([0, 2, 6, 7].map(|byte| map.unit_of(byte)), [0, 1, 3, 4]);
        assert_eq!(map.byte_of(1.0, 0), Some(2));
        assert_eq!(map.byte_of(3.0, 0), Some(6));
        assert_eq!(map.byte_of(4.0, 0), Some(7));
        // Inside the pair, past the end, before the start, not a whole number.
        assert_eq!(map.byte_of(2.0, 0), None);
        assert_eq!(map.byte_of(5.0, 0), None);
        assert_eq!(map.byte_of(1.0, 3), None);
        assert_eq!(map.byte_of(1.5, 0), None);
    }

    #[test]
    fn ascii_is_identity_up_to_the_length() {
        let map = OffsetMap::new("abc");
        assert_eq!(map.unit_of(2), 2);
        assert_eq!(map.byte_of(3.0, 1), Some(3));
        assert_eq!(map.byte_of(4.0, 1), None);
    }
}
