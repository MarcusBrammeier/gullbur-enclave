import App from "./App.svelte";
import { mount } from "svelte";
import { installConsoleBridge } from "./lib/consoleBridge";

// Install the console bridge as early as possible so WebView errors, logs and
// unhandled rejections are captured from the very first tick — before the Svelte
// tree mounts — and forwarded to the in-app Debug Console + Rust/terminal.
installConsoleBridge();

const app = mount(App, {
  target: document.getElementById("app")!,
});

export default app;
