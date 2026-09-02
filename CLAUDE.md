# Notefix — project conventions

Notefix ships from one codebase as a **desktop app** (Tauri, macOS/Windows/Linux)
and **mobile apps** (Tauri iOS/Android). Keep both working.

Notefix also has a **protected notes vault**: individual notes and whole
folders can be encrypted at rest (XChaCha20-Poly1305), with the DEK wrapped by
an Argon2id passphrase, a recovery key, and optionally the OS keychain for
Touch ID unlock. Protected content syncs as ciphertext; cross-device sync of
the vault record itself is a documented follow-up.

## Desktop vs. mobile capabilities

Some backend commands are desktop-only (`#[cfg(desktop)]` in `src-tauri`):
window/tray control, autostart, the updater, single-instance, multi-window,
the folder picker (storage-location change), and the close-to-tray behavior.
On mobile these commands don't exist and calling them fails.

**Rule — when adding or reviewing any setting or feature that calls a Tauri
command, check whether that command is `#[cfg(desktop)]`. If it is, hide the UI
on mobile.** Gate it with `isMobilePlatform` from `src/platform.ts`.

Already gated (keep this list current): update check (About), autostart /
start-minimized / close-behavior / storage-location (System), the autostart +
window diagnostics checks, and the MCP nav item (the MCP server targets local
desktop AI clients, not phones).

The vault's biometric unlock (`vault_biometric_available` / `_enable` /
`_disable`, `vault_unlock_biometric`) is macOS-desktop-only: it's backed by
Touch ID via `LocalAuthentication`. On mobile and Linux `is_available()`
simply returns `false` (mobile biometric via a plugin is deferred), so gate
the biometric UI on that check rather than on `isMobilePlatform` alone.

The biometric keychain item is **per context** (`vault-dek:<context id>`) —
every context is its own vault with its own DEK, so an app-wide item would
hand one context another's key. The `VaultRecord` carries a `dek_check`
(magic sealed under the DEK); every unlock path verifies a candidate DEK
against it before installing it, and records that predate the check gain it
on their next passphrase/recovery unlock. The pre-per-context item is
deleted once at startup; users re-enable Touch ID per context.

For a **server context** the vault belongs to the workspace: the server keeps
one wrapped copy of each key generation per member (`workspace_vault_keys`),
the recovery wrap for the vault's creator, and pending invite/rotation wraps.
`VaultState` is a ring `generation → DEK`; new content is sealed with the
newest generation, notes remember theirs in `notes.key_gen`, and an unlocked
client re-seals lagging notes in small batches. A server without the vault
endpoints (`vaultKeys` missing on pull) is flagged `vault_server_legacy` and
keeps the local-only vault. Never send a DEK, passphrase, recovery key or
invite code to the server — only wraps.

### Two different "mobile" signals — don't confuse them

- `useIsMobile()` (`src/hooks/useIsMobile.ts`) — **viewport width** (≤640px).
  Drives responsive *layout* (single-column, larger touch targets). A narrow
  desktop window is `isMobile` and *should* get the mobile layout.
- `isMobilePlatform` (`src/platform.ts`) — **the OS** (Android/iOS), from the
  WebView user agent. Gates desktop-only *capabilities*. A narrow desktop
  window is NOT `isMobilePlatform` and keeps its desktop features.

## Mobile layout

- Single-column below 640px: note list and editor share the viewport with a
  back button; settings use the same nav→page drill-down.
- Respect safe areas: top bars/columns pad with `env(safe-area-inset-top)`,
  bottom bars with `env(safe-area-inset-bottom)`. `env()` is 0 on desktop, so
  applying it unconditionally is a no-op there.
- `index.html` must keep `viewport-fit=cover` for the insets to resolve.

## Build / verify

- `npx tsc --noEmit` and `npx vitest run` must stay green (i18n de/en/fr key
  parity is enforced by a test).
- Coverage: `npm run test:coverage` (frontend, v8; all `src/` files are
  reported, `main.tsx` excluded) and, from `src-tauri/`,
  `cargo llvm-cov --lib --summary-only -- --test-threads=1` (needs
  `cargo-llvm-cov` + the `llvm-tools-preview` rustup component; the
  instrumented binary is SIGKILLed under default test parallelism, hence
  `--test-threads=1`). Both are GATES, not just reports: `npm run
  test:coverage` fails below 90% lines / 88% statements + functions / 80%
  branches (`vitest.config.ts` thresholds), and `npm run test:coverage:rust`
  fails below 75% lines (`--fail-under-lines`). Current level: frontend ~96%
  lines, backend ~79% lines — keep both gates green when adding code.
- Keep the backend testable: `commands.rs` is a THIN Tauri surface (acquire
  `State` locks, delegate, emit events). Real logic — validation, branching,
  store/vault mutations — lives as pure functions in `ops.rs` and is
  unit-tested there with `Store::open_in_memory()`. New command logic goes in
  `ops.rs`, not in the command body. `commands.rs`/`lib.rs` being near 0%
  covered is expected (macro expansion + app builder), not a gap.
- Android debug APK: `npx tauri android build --debug --apk --target aarch64`.
  Always delete `src-tauri/gen/android/app/build/outputs/apk` (or `gradlew
  clean`) first — rebuilding over an existing APK appends instead of rewriting
  and doubles its size.
- Play Store release (signed AAB): `npx tauri android build --aab` → output at
  `src-tauri/gen/android/app/build/outputs/bundle/universalRelease/app-universal-release.aab`.
  Signing is wired in `app/build.gradle.kts` via the gitignored
  `src-tauri/gen/android/keystore.properties` (upload keystore `~/notefix-upload.jks`,
  alias `notefix`). Bump `tauri.android.versionCode` in `tauri.properties` before
  every Play upload. Never commit the keystore or its password.
- Android toolchain: SDK at `~/Library/Android/sdk` (installed NDK
  `27.2.12479018`, platform android-36, build-tools 36.0.0). JDK 17 via
  `brew install openjdk@17` (keg-only) — builds need
  `export JAVA_HOME="/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home"`.
  `adb` is at `$ANDROID_HOME/platform-tools/adb` (not on PATH).
