# Phextron

Phextron tooling (Will, 2026-11-21: Rust only, no C#, no NuGet).

## Crates

- `crates/gl1-http` — G-L1: lineage HTTP+TLS client for phext-lattice (ureq + rustls, mirroring lattice's own Cargo.toml). See its README for the full dependency enumeration.
- `crates/gl2-credman` — G-L2: thin Windows Credential Manager binding via direct advapi32/shell32 FFI (zero dependencies). See its README.

- `crates/phextron-dash` — the console dashboards: one shell, one live-source panel per domain (ranch nodes live in stage 1; Jenkins, video, cloud AI, phexts per docs/RECON.md), plus the Data panel: every data stash in the estate, read from the stash.phext census register.
