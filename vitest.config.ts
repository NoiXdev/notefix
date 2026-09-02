import { defineConfig, configDefaults } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    // e2e/ holds Playwright specs (run via `npm run test:e2e`); keep them out of vitest.
    // .claude/ can hold agent worktrees (full repo checkouts) whose duplicate
    // test files would otherwise be collected and run against a stale state.
    exclude: [...configDefaults.exclude, "e2e/**", ".claude/**"],
    // `npm run test:coverage`. Every src file is reported, tested or not, so
    // the numbers reflect the whole frontend rather than only imported files.
    // main.tsx is the React mount point (no logic) and is deliberately left out.
    coverage: {
      provider: "v8",
      include: ["src/**/*.{ts,tsx}"],
      exclude: ["src/**/*.test.{ts,tsx}", "src/test/**", "src/main.tsx", "src/**/*.d.ts"],
      reporter: ["text-summary", "text", "html"],
      reportsDirectory: "coverage",
      // Gate: `npm run test:coverage` fails below these floors (set a little
      // under the post-coverage-program level so ordinary churn doesn't trip
      // them, but a real regression does).
      thresholds: { lines: 90, statements: 88, functions: 88, branches: 80 },
    },
  },
});
