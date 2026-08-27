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
- Android debug APK: `npx tauri android build --debug --apk --target aarch64`.
  Always delete `src-tauri/gen/android/app/build/outputs/apk` (or `gradlew
  clean`) first — rebuilding over an existing APK appends instead of rewriting
  and doubles its size.
- Android toolchain lives at `/opt/homebrew/share/android-commandlinetools`
  (JDK 17 at `/opt/homebrew/opt/openjdk@17`); `adb` is at
  `$ANDROID_HOME/platform-tools/adb` (not on PATH).
