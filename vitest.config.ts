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
  },
});
