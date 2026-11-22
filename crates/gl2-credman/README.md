# gl2-credman — Phextron G-L2

Thin Windows Credential Manager binding. Direct FFI ONLY: hand-declared
`extern "system"` bindings to advapi32 (CredWriteW, CredReadW, CredDeleteW,
CredFree) and shell32 (ShellExecuteW), with `#[link]` directives for
advapi32/shell32/kernel32. NO `windows` crate, NO keyring-class abstraction
(SPEC §6.2 strikes them).

API: `store(target, blob)`, `read(target) -> Result<Option<Vec<u8>>>`,
`delete(target)`, `open_url(url)` (ShellExecuteW, for the SSO browser leg).
The generic-credential blob limit (2560 bytes) is enforced with a typed
`Error::BlobTooLarge`. FFI is cfg-gated to Windows; on other platforms the
same API returns typed `Error::Unsupported` so the crate compiles and its
platform-independent tests run on Linux.

## Dependency enumeration

Direct dependencies: **none** (std only). Transitive dependencies: **none**.

```text
gl2-credman v0.1.0 (/home/wbic16/phextron-gl-build/crates/gl2-credman)
```

Round-trip: `credman-roundtrip` bin stores, reads back, compares, and deletes
a test blob under target `phextron-gl2-test/roundtrip`. Verified by
cross-compiling for x86_64-pc-windows-gnu and running under Wine, and by the
Jenkins job `phextron-gl2-windows-x64`.
