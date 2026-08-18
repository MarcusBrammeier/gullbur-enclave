# Building from Source

How to build **Gullbúr Enclave** from this repository. This is the exact workflow
used for the release builds, so it is the *community-supported* path.

## Prerequisites

- **Rust** — stable toolchain, **edition 2024** (`rustup update stable`). Nightly
  is only needed for fuzzing.
- **Node.js + npm** — for the web frontend (used by the desktop shell).
- **Linux build deps** for the Tauri desktop app: `libwebkit2gtk-4.1-dev`,
  `build-essential`, `curl`, `wget`, `file`, `libxdo-dev`, `libssl-dev`,
  `libayatana-appindicator3-dev`, `librsvg2-dev`.
- **Android SDK** (only if building the Android APK/AAB) — see
  `apps/desktop/src-tauri/gen` and the `tauri android` docs.

> **Always use `--locked` for Cargo builds in this repo.**
> The workspace intentionally pins a dual `sha2` (0.10 + 0.11) dependency
> resolution. A plain `cargo build` (without `--locked`) can re-resolve and
> collapse that resolution, breaking `keystore-core`'s HKDF at compile time.
> `--locked` uses the committed `Cargo.lock` verbatim and avoids this.

## Desktop app (Linux AppImage / .deb)

```bash
# 1. Install the frontend dependencies
cd apps/desktop
npm ci

# 2. Build the web frontend (needed before the Rust build — the Tauri
#    generate_context! macro requires apps/desktop/dist to exist)
npm run build

# 3. Build the Rust desktop application
cd ../..
cargo build --release -p gullbur-desktop --locked
```

The binary is at `target/release/gullbur-desktop`. To produce the bundle
(`.deb` / `.AppImage`):

```bash
cd apps/desktop
npx tauri build --bundles deb,appimage
```

## Library tests

```bash
cargo test --workspace --lib --locked
```

This runs the full crate test suite (API tests, crypto round-trips, plugin
logic). The live-network integration tests are `#[ignore]`d and require real
testnet/stagenet funding — see [TESTING.md](TESTING.md).

## Fuzzing (optional, requires nightly Rust)

```bash
cargo +nightly fuzz build --fuzz-dir fuzz
cargo +nightly fuzz run --fuzz-dir fuzz fuzz_json_rpc -- -max_total_time=30
```

## Android (optional)

```bash
cd apps/desktop
ANDROID_HOME=/path/to/android-sdk ANDROID_NDK_HOME=/path/to/ndk \
  npx tauri android build --apk --aab
```

---

### What's included / excluded

This **public** repository ships the core engine, the desktop GUI, the browser
extension, and the integration/fuzz tests. It deliberately **excludes** the
`gullbur-cli` headless tool, the internal test sweeps, and internal planning
docs — those live in the private development repo.