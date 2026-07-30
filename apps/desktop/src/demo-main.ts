/**
 * Demo entry point — same App component as production, but with MockIpcClient.
 * Run: VITE_DEMO=true npx vite --host
 */
import App from "./App.svelte";
import { mount } from "svelte";

// Inject demo flag so vault.svelte.ts picks up MockIpcClient
(window as any).__DEMO__ = true;

const app = mount(App, {
  target: document.getElementById("app")!,
});

export default app;