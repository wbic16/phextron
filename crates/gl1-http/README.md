# gl1-http — Phextron G-L1

Lineage HTTP+TLS client for phext-lattice, mirroring phext-lattice's own
Cargo.toml choices (SPEC §6.1, lattice @ exo 2ba5710f): `ureq` version "2",
`default-features = false`, `features = ["tls"]` (rustls TLS). JSON only at
this boundary, because lattice already speaks it there (`/api/session`, the
desktop exchange).

API: `LatticeClient { base_url }` with generic `get(path)` /
`post(path, body)` returning `(status, body)`; session presented as the
`phext_session` cookie; CSRF sent as `x-csrf-token` on writes; typed helpers
`exchange(assertion, state)` (POST /api/auth/desktop/exchange with
`x-phext-assertion` / `x-phext-state`, returning and storing the
`{session, csrf}` pair) and `session()` (GET /api/session).

## Dependency enumeration (lineage rule: every chain enumerated)

Direct dependencies — complete list, each with its why:

- `ureq` 2 (default-features off, tls) — the HTTP+TLS client itself; the exact choice phext-lattice already makes, small and enumerable, rustls TLS. A reqwest-class tree is struck by SPEC §5/§6.
- `serde` 1 (derive) — typed (de)serialization of the exchange `{session, csrf}` pair at the lattice JSON boundary; already in lattice's tree.
- `serde_json` 1 — parsing lattice's JSON boundary responses; already in lattice's tree.

Nothing else. No other direct dependency is taken, per the task rule.

Transitive dependencies: the full `cargo tree` output below is the
enumeration of record. Unique transitive crates (excluding gl1-http itself)
are counted in the commit/report that introduced this crate.

```text
gl1-http v0.1.0 (/home/wbic16/phextron-gl-build/crates/gl1-http)
├── serde v1.0.229
│   ├── serde_core v1.0.229
│   └── serde_derive v1.0.229 (proc-macro)
│       ├── proc-macro2 v1.0.107
│       │   └── unicode-ident v1.0.27
│       ├── quote v1.0.47
│       │   └── proc-macro2 v1.0.107 (*)
│       └── syn v3.0.10
│           ├── proc-macro2 v1.0.107 (*)
│           ├── quote v1.0.47 (*)
│           └── unicode-ident v1.0.27
├── serde_json v1.0.154
│   ├── itoa v1.0.19
│   ├── memchr v2.8.3
│   ├── serde_core v1.0.229
│   └── zmij v1.0.23
└── ureq v2.12.1
    ├── base64 v0.22.1
    ├── log v0.4.35
    ├── once_cell v1.21.4
    ├── rustls v0.23.46
    │   ├── log v0.4.35
    │   ├── once_cell v1.21.4
    │   ├── ring v0.17.14
    │   │   ├── cfg-if v1.0.5
    │   │   ├── getrandom v0.2.17
    │   │   │   ├── cfg-if v1.0.5
    │   │   │   └── libc v0.2.191
    │   │   └── untrusted v0.9.0
    │   │   [build-dependencies]
    │   │   └── cc v1.11.3
    │   │       ├── find-msvc-tools v0.1.15
    │   │       └── shlex v2.0.1
    │   ├── rustls-pki-types v1.16.0
    │   │   └── zeroize v1.9.2
    │   ├── rustls-webpki v0.103.17
    │   │   ├── ring v0.17.14 (*)
    │   │   ├── rustls-pki-types v1.16.0 (*)
    │   │   └── untrusted v0.9.0
    │   ├── subtle v2.6.1
    │   └── zeroize v1.9.2
    ├── rustls-pki-types v1.16.0 (*)
    ├── url v2.5.9
    │   ├── form_urlencoded v1.2.3
    │   │   └── percent-encoding v2.3.2
    │   ├── idna v1.1.0
    │   │   ├── idna_adapter v1.2.2
    │   │   │   ├── icu_normalizer v2.4.0
    │   │   │   │   ├── icu_collections v2.4.0
    │   │   │   │   │   ├── displaydoc v0.2.7 (proc-macro)
    │   │   │   │   │   │   ├── proc-macro2 v1.0.107 (*)
    │   │   │   │   │   │   ├── quote v1.0.47 (*)
    │   │   │   │   │   │   └── syn v3.0.10 (*)
    │   │   │   │   │   ├── potential_utf v0.1.6
    │   │   │   │   │   │   └── zerovec v0.11.9
    │   │   │   │   │   │       ├── yoke v0.8.4
    │   │   │   │   │   │       │   ├── stable_deref_trait v1.2.1
    │   │   │   │   │   │       │   ├── yoke-derive v0.8.4 (proc-macro)
    │   │   │   │   │   │       │   │   ├── proc-macro2 v1.0.107 (*)
    │   │   │   │   │   │       │   │   ├── quote v1.0.47 (*)
    │   │   │   │   │   │       │   │   ├── syn v3.0.10 (*)
    │   │   │   │   │   │       │   │   └── synstructure v0.14.0
    │   │   │   │   │   │       │   │       ├── proc-macro2 v1.0.107 (*)
    │   │   │   │   │   │       │   │       ├── quote v1.0.47 (*)
    │   │   │   │   │   │       │   │       └── syn v3.0.10 (*)
    │   │   │   │   │   │       │   └── zerofrom v0.1.8
    │   │   │   │   │   │       │       └── zerofrom-derive v0.1.8 (proc-macro)
    │   │   │   │   │   │       │           ├── proc-macro2 v1.0.107 (*)
    │   │   │   │   │   │       │           ├── quote v1.0.47 (*)
    │   │   │   │   │   │       │           ├── syn v3.0.10 (*)
    │   │   │   │   │   │       │           └── synstructure v0.14.0 (*)
    │   │   │   │   │   │       ├── zerofrom v0.1.8 (*)
    │   │   │   │   │   │       └── zerovec-derive v0.11.7 (proc-macro)
    │   │   │   │   │   │           ├── proc-macro2 v1.0.107 (*)
    │   │   │   │   │   │           ├── quote v1.0.47 (*)
    │   │   │   │   │   │           └── syn v3.0.10 (*)
    │   │   │   │   │   ├── utf8_iter v1.1.0
    │   │   │   │   │   │   └── utf_types v1.0.0
    │   │   │   │   │   ├── yoke v0.8.4 (*)
    │   │   │   │   │   ├── zerofrom v0.1.8 (*)
    │   │   │   │   │   └── zerovec v0.11.9 (*)
    │   │   │   │   ├── icu_normalizer_data v2.4.0
    │   │   │   │   ├── icu_provider v2.4.0
    │   │   │   │   │   ├── displaydoc v0.2.7 (proc-macro) (*)
    │   │   │   │   │   ├── icu_locale_core v2.4.0
    │   │   │   │   │   │   ├── displaydoc v0.2.7 (proc-macro) (*)
    │   │   │   │   │   │   ├── litemap v0.8.3
    │   │   │   │   │   │   ├── tinystr v0.8.5
    │   │   │   │   │   │   │   ├── displaydoc v0.2.7 (proc-macro) (*)
    │   │   │   │   │   │   │   └── zerovec v0.11.9 (*)
    │   │   │   │   │   │   ├── writeable v0.6.4
    │   │   │   │   │   │   └── zerovec v0.11.9 (*)
    │   │   │   │   │   ├── writeable v0.6.4
    │   │   │   │   │   ├── yoke v0.8.4 (*)
    │   │   │   │   │   ├── zerofrom v0.1.8 (*)
    │   │   │   │   │   ├── zerotrie v0.2.5
    │   │   │   │   │   │   ├── displaydoc v0.2.7 (proc-macro) (*)
    │   │   │   │   │   │   ├── yoke v0.8.4 (*)
    │   │   │   │   │   │   └── zerofrom v0.1.8 (*)
    │   │   │   │   │   └── zerovec v0.11.9 (*)
    │   │   │   │   ├── smallvec v1.16.6
    │   │   │   │   └── zerovec v0.11.9 (*)
    │   │   │   └── icu_properties v2.4.0
    │   │   │       ├── displaydoc v0.2.7 (proc-macro) (*)
    │   │   │       ├── icu_collections v2.4.0 (*)
    │   │   │       ├── icu_locale_core v2.4.0 (*)
    │   │   │       ├── icu_properties_data v2.4.0
    │   │   │       ├── icu_provider v2.4.0 (*)
    │   │   │       ├── zerotrie v0.2.5 (*)
    │   │   │       └── zerovec v0.11.9 (*)
    │   │   ├── smallvec v1.16.6
    │   │   └── utf8_iter v1.1.0 (*)
    │   └── percent-encoding v2.3.2
    └── webpki-roots v0.26.11
        └── webpki-roots v1.0.12
            └── rustls-pki-types v1.16.0 (*)
```

Tests: unit tests for header/cookie construction and URL joining; a stub-server
integration test (std TcpListener) exercising get/post/exchange shapes; an
env-gated live test (`GL1_LIVE=1`) against https://edit.phext.io/api/session.
