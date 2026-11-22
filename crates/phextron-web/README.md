# phextron-web — Phextron's web UI, rendered via WASM

Task 33.12.9/13.10.15/11.10.12 · W4 plan §1.12 · Will's direction
(2026-11-21): *"Phextron should have a web UI as well - rendered via WASM"*

**Canonical host: tron.phext.io** (registered by Will, 2026-11-21 23:19 CST).
**Auth: phext.tech single sign-on** (Will's ruling, 2026-11-21 23:20 CST) —
tron.phext.io consumes phext.tech's SSO; this surface runs no auth of its
own and invents none. The integration point is named in `web/index.html`
(`#sso-slot`); the wiring itself is a stage-2 step (below).

## Stage 1 — what this is

One codebase, two render targets. The desktop console's panels are defined
by the JSON their readers serve at `/api/panels/<id>`; this crate compiles
the console's *rendering* code — the panel renderers, in Rust — to
WebAssembly, so a browser renders the same panels from the same live JSON.

Stage 1 proves the target, it does not port the fleet:

- the wasm32 build pipeline (this crate + `build.sh`),
- the shell page that loads the module (`web/index.html`, `web/glue.js`),
- ONE panel end-to-end: **ranch nodes** (`src/render.rs` is a line-for-line
  port of the desktop shell's `renderNodes`, same HTML and classes, plus
  the `renderDegraded` port every panel needs). Nodes was picked because
  it is the console's founding live panel — the most-exercised data path,
  a bounded renderer, and the panel the desktop stage-1 browser check
  already asserts (23 roster rows), so the WASM check asserts against the
  same reality.

Deliberately not in stage 1: the other panels (Jenkins, Video, Cloud AI,
All Phexts, Data — and the Substrates/Wiring panels landing alongside),
tab navigation across panels, any deployment, and SSO wiring. Stage 1
served locally is unauthenticated, like the desktop console's stage 1;
that is a stated limitation, not an oversight.

## Toolchain choice — measured against the human-slop rule

**Raw `wasm32-unknown-unknown` + a hand-written JS glue. Dependency count
added: 0 crates.** `cargo tree` for this crate lists exactly one package:
phextron-web itself.

- The module is a pure function (panel JSON bytes in → HTML bytes out)
  behind a three-function ABI (`pw_alloc` / `pw_render` / `pw_free`), so
  no binding generator is needed: the glue is ~100 lines of plain JS.
- JSON parsing is a small std-only recursive-descent parser (`src/json.rs`)
  instead of serde_json — serde would have added serde + serde_json +
  itoa + ryu + memchr (5 crates) to the browser build's lineage for one
  parse. Same call the Data panel made with its std-only SHA-256.
- What the light path genuinely cannot do, stated plainly: direct DOM
  construction from Rust (the module returns HTML strings, exactly as the
  desktop shell's renderers build HTML strings for `innerHTML` — parity,
  not a workaround), and ergonomic async (fetch lives in the glue; the
  module never blocks). If a future panel needs true DOM diffing, that is
  the moment to re-measure wasm-bindgen/web-sys against the rule — with
  the count in hand, not by default.

Because the module is pure, the whole render path is host-testable:
`cargo test` covers the parser and the nodes renderer (rows, chips,
banner, escaping, degraded, garbage input). The browser pass
(`verify_browser.py`) then only proves load + execution + live data.

## Build & serve

```sh
rustup target add wasm32-unknown-unknown   # once
./build.sh                                  # → dist/
cargo test                                  # host tests

# serve through the console (routes land in crates/phextron-dash):
PHEXTRON_WEB_DIR=$PWD/dist phextron         # then open /web
```

Dark mode follows the OS preference by default with the visitor's
persisted override — the desktop shell's exact pre-paint script, storage
key (`phextron.theme`), and theme variables, so the two surfaces share
one choice and one look.

## Stage 2 — recommended order (for the next builder)

1. **Deploy stage 1 to tron.phext.io**: awsbox vhost + TLS for
   tron.phext.io following the phext.tech launch pattern on the same box
   (the domain already resolves to awsbox, 44.248.235.76; the deploy-time
   staged sudo is Will's step), serving the dash server with
   `PHEXTRON_WEB_DIR` at this crate's `dist/`.
2. **Wire phext.tech SSO**: the sign-in control renders into `#sso-slot`
   in `web/index.html` and the panel API rides the phext.tech session.
   No tron-local credential system, per the ruling.
3. **Port the panels in the desktop shell's own order of liveness**:
   Jenkins next (bounded tables, live), then All Phexts, Video, Data
   (the nested-rows renderer is the most intricate — port it last of the
   live set), then Substrates and Wiring as their desktop panels settle;
   Cloud AI when its reader leaves recon. Each port is one renderer in
   `src/render.rs` + its fixture test, behind the shell's tab bar once
   two or more panels render.
4. Retire the duplicated CSS by generating both shells' styles from one
   source once the tab bar lands — until then the copy in
   `web/index.html` is deliberate parity, kept honest by the side-by-side
   browser screenshots.
