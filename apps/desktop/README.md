# Gullbúr Enclave — Desktop App

Tauri v2 desktop application (Rust + Svelte 5).

## AppImage Build

### Known Issue: libgcrypt.so.20 Runtime Crash

**Symptom:** The AppImage crashes at launch with:

```
error while loading shared libraries: libgcrypt.so.20: cannot open shared object file
```

**Root Cause:** The linuxdeploy AppImage bundler does not automatically bundle `libgcrypt.so.20` and its transitive dependency `libgpg-error.so.0`. At runtime, the dynamic linker inside the AppImage sandbox cannot find these libraries.

**Fix Applied (Aug 2026) — `tauri.conf.json`:**

Three changes were made to the AppImage bundle configuration in `src-tauri/tauri.conf.json`:

1. **`bundleMediaFramework: true`** — Enables the linuxdeploy GTK/media-framework plugin, which discovers and bundles all GTK-related shared library dependencies (including libgcrypt and libgpg-error).

2. **Explicit library bundling via `files`** — `libgcrypt.so.20` and `libgpg-error.so.0` are explicitly copied into the AppImage under `usr/lib/` (the standard library path that AppRun adds to `LD_LIBRARY_PATH`).

3. **Deb package dependency declaration** — `libgcrypt20` and `libgpg-error0` were added to the `deb.depends` list so `.deb` packages declare the dependency correctly.

### Runtime Workaround (if crash persists)

If the AppImage still crashes at runtime despite the build fixes, bypass the FUSE mount layer:

```bash
APPIMAGE_EXTRACT_AND_RUN=1 ./Gullbur*.AppImage
```

This extracts the AppImage to a temporary directory and runs it from there, avoiding FUSE/libgcrypt runtime-link issues entirely.

### Build Command

```bash
npm run tauri:build
```
