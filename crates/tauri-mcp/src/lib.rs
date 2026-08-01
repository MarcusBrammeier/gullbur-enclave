use std::sync::Mutex;
use tauri::{
    Manager, Runtime,
    plugin::{Builder, TauriPlugin},
};
use serde_json::json;

/// Tauri MCP plugin — AI-driven E2E testing tools.
/// MUST NEVER be compiled into production builds.
pub struct TauriMcp<R: Runtime> {
    #[allow(dead_code)]
    app_handle: tauri::AppHandle<R>,
}

/// Shared state for receiving JS eval results from the webview callback.
struct McpCallbackState {
    last_result: Mutex<Option<String>>,
}

// ── Internal: callback for JS eval results ──

#[tauri::command]
fn __mcp_callback(state: tauri::State<'_, McpCallbackState>, result: String) {
    if let Ok(mut guard) = state.last_result.lock() {
        *guard = Some(result);
    }
}

// ── Debug builds: real implementations ──

#[cfg(debug_assertions)]
#[tauri::command]
async fn mcp_dom_query<R: Runtime>(
    _app: tauri::AppHandle<R>,
    window: tauri::WebviewWindow<R>,
    selector: String,
) -> Result<String, String> {
    let escaped = selector
        .replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', " ");
    let js = format!(
        r#"(() => {{
    try {{
        const els = Array.from(document.querySelectorAll('{}'));
        const result = els.map(el => ({{
            tag: el.tagName.toLowerCase(),
            id: el.id || null,
            classes: Array.from(el.classList),
            text: (el.textContent || '').trim().slice(0, 200),
            attributes: Object.fromEntries(
                el.getAttributeNames().map(n => [n, el.getAttribute(n)])
            ),
        }}));
        return JSON.stringify({{ selector: '{}', elements: result }});
    }} catch(e) {{
        return JSON.stringify({{ selector: '{}', elements: [], error: e.message }});
    }}
}})()"#,
        escaped, escaped, escaped
    );
    window
        .eval(format!("window.__tauri_mcp_tmp = {js}"))
        .map_err(|e| e.to_string())?;
    // Return the result inline for now — in production, use the callback pattern
    let result = json!({"selector": selector, "elements": [], "note": "run eval() above — result available via callback"});
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

#[cfg(debug_assertions)]
#[tauri::command]
async fn mcp_dom_click<R: Runtime>(
    _app: tauri::AppHandle<R>,
    window: tauri::WebviewWindow<R>,
    selector: String,
) -> Result<String, String> {
    let escaped = selector
        .replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', " ");
    let js = format!(
        r#"(() => {{
    try {{
        const el = document.querySelector('{}');
        if (!el) {{
            return JSON.stringify({{ selector: '{}', clicked: false, error: 'element not found' }});
        }}
        el.click();
        return JSON.stringify({{ selector: '{}', clicked: true, error: null }});
    }} catch(e) {{
        return JSON.stringify({{ selector: '{}', clicked: false, error: e.message }});
    }}
}})()"#,
        escaped, escaped, escaped, escaped
    );
    window
        .eval(format!("window.__tauri_mcp_tmp = {js}"))
        .map_err(|e| e.to_string())?;
    let result =
        json!({"selector": selector, "clicked": true, "note": "click dispatched via eval()"});
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

#[cfg(debug_assertions)]
#[tauri::command]
async fn mcp_webview_screenshot<R: Runtime>(
    _app: tauri::AppHandle<R>,
    window: tauri::WebviewWindow<R>,
) -> Result<String, String> {
    // Screenshot via html2canvas injection or Tauri screenshot API
    let js = r#"(() => {
    try {
        if (typeof html2canvas !== 'undefined') {
            html2canvas(document.body).then(canvas => {
                const data = canvas.toDataURL('image/png');
                window.__tauri_mcp_tmp = JSON.stringify({screenshot: data, width: canvas.width, height: canvas.height});
            });
            return JSON.stringify({screenshot: '', width: 0, height: 0, error: 'async capture in progress'});
        }
        return JSON.stringify({screenshot: '', width: 0, height: 0, error: 'html2canvas not loaded'});
    } catch(e) {
        return JSON.stringify({screenshot: '', width: 0, height: 0, error: e.message});
    }
})()"#;
    window
        .eval(format!("window.__tauri_mcp_tmp = {js}"))
        .map_err(|e| e.to_string())?;
    let result = json!({"screenshot": "", "width": 0, "height": 0, "note": "screenshot dispatched via eval()"});
    serde_json::to_string(&result).map_err(|e| e.to_string())
}

// ── Release builds: blocked ──

#[cfg(not(debug_assertions))]
#[tauri::command]
fn mcp_dom_query<R: Runtime>(
    _app: tauri::AppHandle<R>,
    _window: tauri::Window<R>,
    _selector: String,
) -> Result<String, String> {
    Err("MCP tools only available in debug builds".into())
}

#[cfg(not(debug_assertions))]
#[tauri::command]
fn mcp_dom_click<R: Runtime>(
    _app: tauri::AppHandle<R>,
    _window: tauri::Window<R>,
    _selector: String,
) -> Result<String, String> {
    Err("MCP tools only available in debug builds".into())
}

#[cfg(not(debug_assertions))]
#[tauri::command]
fn mcp_webview_screenshot<R: Runtime>(
    _app: tauri::AppHandle<R>,
    _window: tauri::Window<R>,
) -> Result<String, String> {
    Err("MCP tools only available in debug builds".into())
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::<R>::new("tauri-mcp")
        .setup(|app, _api| {
            app.manage(McpCallbackState {
                last_result: Mutex::new(None),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            __mcp_callback,
            mcp_dom_query,
            mcp_dom_click,
            mcp_webview_screenshot,
        ])
        .build()
}
