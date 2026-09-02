import { describe, it, expect } from 'vitest';
import { resolveDefaultExport } from './interop';

// The shapes a TypeScript-compiled CJS package (`exports.default = Thing`) can
// take after ESM interop. The double-wrapped one is what Vite's browser bundle
// produces for `import * as ns` — and what made the markdown view crash with
// "Element type is invalid … got: object" while vitest (which hands over the
// bare function) kept every test green.
function Thing() {
  return null;
}

describe('resolveDefaultExport', () => {
  it('returns a bare function untouched', () => {
    expect(resolveDefaultExport<typeof Thing>(Thing)).toBe(Thing);
  });

  it('unwraps the raw module.exports shape', () => {
    const mod = { default: Thing, __esModule: true };
    expect(resolveDefaultExport<typeof Thing>(mod)).toBe(Thing);
  });

  it("unwraps Vite's double-wrapped namespace (namespace.default = module.exports)", () => {
    const ns = { default: { default: Thing, __esModule: true }, __esModule: true };
    expect(resolveDefaultExport<typeof Thing>(ns)).toBe(Thing);
  });

  it('returns an object with no default export as-is', () => {
    const mod = { named: 1 };
    expect(resolveDefaultExport<typeof mod>(mod)).toBe(mod);
  });

  it('stops on a self-referential default instead of looping forever', () => {
    const cyclic: { default?: unknown } = {};
    cyclic.default = cyclic;
    expect(resolveDefaultExport<unknown>(cyclic)).toBe(cyclic);
  });
});
