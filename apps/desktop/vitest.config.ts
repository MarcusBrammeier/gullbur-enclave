import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.ts"],
    setupFiles: ["src/test/setup.ts"],
  },
  resolve: {
    // Force the browser (client) Svelte build so @testing-library/svelte's
    // mount()/render() works — the server build throws lifecycle_function_unavailable.
    conditions: ["browser"],
  },
});
