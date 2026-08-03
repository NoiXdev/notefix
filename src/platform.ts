/**
 * Runtime platform detection for gating features that only exist on desktop.
 *
 * This is deliberately about the OS (Android/iOS vs. desktop), NOT the viewport
 * width — a narrow desktop window is still desktop and keeps its window/tray/
 * autostart/updater features. Use `useIsMobile` for responsive *layout*; use
 * this for hiding desktop-only *capabilities*.
 *
 * Detected from the WebView user agent so it stays synchronous and needs no
 * extra Tauri plugin. In jsdom (unit tests) the UA is not mobile, so tests see
 * the full desktop UI.
 */
export const isMobilePlatform =
  typeof navigator !== 'undefined' && /android|iphone|ipad|ipod/i.test(navigator.userAgent);
