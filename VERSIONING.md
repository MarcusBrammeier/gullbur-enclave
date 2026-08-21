# Versioning

Gullbúr Enclave uses SemVer for all releases.

## Bump rules

- **Bug fix** → bump **patch**: `0.1.0` → `0.1.1`
- **Feature / function upgrade** → bump **minor**: `0.1.0` → `0.2.0`
- **Breaking change** → bump **major**: `1.0.0`

## Public source of truth

The version is stored in:

| File | Field |
|------|-------|
| `Cargo.toml` (workspace root) | `version = "0.1.0"` |
| each crate `crates/*/Cargo.toml` | `version = "0.1.0"` |
| `Cargo.lock` | workspace-member `[[package]]` `version` fields |
| `apps/desktop/package.json` | `"version": "0.1.0"` |
| `apps/desktop/src-tauri/tauri.conf.json` | `"version": "0.1.0"` |

Generated/build output is not hand-edited; it is reproduced on build.

## Releases

Release artifacts (`.deb`, `.AppImage`, `.apk`, `.aab`) are attached to GitHub
Releases. They are built locally and uploaded — they do not require CI minutes.