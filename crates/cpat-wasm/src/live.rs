//! How many parse results wasm is still holding for JS. Each result keeps
//! its tree and report in wasm memory until JS calls `free()`; the JS API
//! frees each one as soon as it has read it (`js/taken.mjs`), so between
//! calls this is zero. A leak shows up here, deterministically, long before
//! it shows up as memory.

use std::cell::Cell;

use wasm_bindgen::prelude::*;

thread_local! {
    static LIVE: Cell<u32> = const { Cell::new(0) };
}

/// A field that counts its owner as live until the owner is dropped.
pub(crate) struct Live(());

impl Live {
    pub(crate) fn new() -> Live {
        LIVE.with(|live| live.set(live.get() + 1));
        Live(())
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        LIVE.with(|live| live.set(live.get() - 1));
    }
}

/// Parse and suggestion results not yet freed.
#[wasm_bindgen(js_name = liveResults)]
pub fn live_results() -> u32 {
    LIVE.with(Cell::get)
}
