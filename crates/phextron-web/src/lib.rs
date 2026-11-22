//! Phextron web UI — the console's panels, rendered client-side from the
//! console's own Rust code compiled to WebAssembly.
//!
//! Canonical host: **tron.phext.io** (registered by Will, 2026-11-21).
//! Auth: tron.phext.io consumes **phext.tech single sign-on** — this crate
//! and its shell run no auth of their own and invent none (Will's ruling,
//! 2026-11-21 23:20 CST). Stage 1 serves locally with no auth, as a proof of
//! the target; the SSO wiring is a stage-2 step, named in README.md.
//!
//! Stage 1 scope (W4 plan §1.12, task 33.12.9/13.10.15/11.10.12):
//! - the wasm32 build pipeline for the console's Rust UI code (this crate),
//! - the shell page that loads the module (`web/index.html` + `web/glue.js`),
//! - ONE panel — ranch nodes — rendering end-to-end through the WASM build
//!   from the same live `/api/panels/nodes` JSON the desktop shell reads.
//!
//! Toolchain: raw `wasm32-unknown-unknown` with a hand-written JS glue —
//! no wasm-bindgen, no web framework, **zero dependencies** (the count and
//! rationale are recorded in this crate's README.md, per the human-slop
//! rule). The ABI is three exported functions; the module is a pure
//! function from panel JSON bytes to HTML bytes, which is also what makes
//! it host-testable with plain `cargo test`.

pub mod json;
pub mod render;

/// Render one panel JSON document (the exact bytes `/api/panels/<id>`
/// serves) to HTML. Parse failures render an error card — the web UI names
/// its failures on its face, like every other Phextron surface.
pub fn render_panel_json(input: &str) -> String {
    match json::parse(input) {
        Ok(doc) => render::render_panel(&doc),
        Err(e) => format!(
            "<div class=\"card\"><h2>Panel data unreadable</h2><p class=\"err\">{}</p></div>",
            crate::render::esc(&e)
        ),
    }
}

/// The WASM ABI. Three exports, no imports:
///
/// - `pw_alloc(len) -> ptr` — reserve a buffer in the module's memory.
/// - `pw_render(ptr, len) -> u64` — read the panel JSON at (ptr, len),
///   render it, and return the HTML buffer packed as
///   `(out_ptr << 32) | out_len`.
/// - `pw_free(ptr, len)` — release a buffer from either direction.
///
/// The glue (`web/glue.js`) owns fetching and DOM placement; the module
/// owns parsing and rendering. Memory never crosses the boundary except
/// through these calls.
#[cfg(target_arch = "wasm32")]
mod wasm_abi {
    #[no_mangle]
    pub extern "C" fn pw_alloc(len: u32) -> *mut u8 {
        let mut v = Vec::with_capacity(len as usize);
        let ptr = v.as_mut_ptr();
        std::mem::forget(v);
        ptr
    }

    #[no_mangle]
    pub extern "C" fn pw_free(ptr: *mut u8, len: u32) {
        if ptr.is_null() || len == 0 {
            return;
        }
        unsafe {
            let _ = Vec::from_raw_parts(ptr, 0, len as usize);
        }
    }

    #[no_mangle]
    pub extern "C" fn pw_render(ptr: *const u8, len: u32) -> u64 {
        let bytes = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
        let input = String::from_utf8_lossy(bytes);
        let html = super::render_panel_json(&input).into_bytes();
        // Copy into an exactly-sized buffer before leaking it: pw_free
        // reconstructs Vec::from_raw_parts(ptr, 0, len), which is only
        // sound when capacity == len. A grown Vec's capacity exceeds its
        // length, and freeing it under the wrong capacity aborts in the
        // allocator (caught by the node smoke test, 2026-11-21).
        let mut v = Vec::with_capacity(html.len());
        v.extend_from_slice(&html);
        debug_assert_eq!(v.capacity(), v.len());
        let out_ptr = v.as_mut_ptr() as u64;
        let out_len = v.len() as u64;
        std::mem::forget(v);
        (out_ptr << 32) | out_len
    }
}
