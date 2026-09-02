/**
 * Resolve the real default export of a CommonJS-only dependency, whatever
 * shape the bundler's ESM interop hands us.
 *
 * A TypeScript-compiled CJS package (`exports.default = Thing`) can arrive as:
 * - `Thing` itself (vitest, Node ESM-CJS interop);
 * - `{ default: Thing, __esModule }` — the raw `module.exports`;
 * - `{ default: { default: Thing, __esModule } }` — Vite's namespace, whose
 *   `.default` is the whole `module.exports` object (a double wrap).
 *
 * Only the first is usable as a React element type; rendering either object
 * throws "Element type is invalid … got: object". Walk `.default` until we hit
 * something callable.
 */
export function resolveDefaultExport<T>(mod: unknown): T {
  let current: unknown = mod;
  for (let depth = 0; depth < 4; depth++) {
    if (typeof current === 'function') return current as T;
    if (current && typeof current === 'object' && 'default' in current) {
      current = (current as { default: unknown }).default;
      continue;
    }
    break;
  }
  return current as T;
}
