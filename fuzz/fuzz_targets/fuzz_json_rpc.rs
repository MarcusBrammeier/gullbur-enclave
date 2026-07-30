#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz JSON-RPC message parsing (deserialize, then roundtrip)
    let raw = String::from_utf8_lossy(data);
    if let Ok(msg) = serde_json::from_str::<ipc_protocol::JsonRpcRequest>(&raw) {
        // Roundtrip test — serialize back and re-parse
        if let Ok(json) = serde_json::to_string(&msg) {
            let _parsed: Result<ipc_protocol::JsonRpcRequest, _> = serde_json::from_str(&json);
        }
    }
    if let Ok(resp) = serde_json::from_str::<ipc_protocol::JsonRpcResponse>(&raw)
        && let Ok(json) = serde_json::to_string(&resp)
    {
        let _parsed: Result<ipc_protocol::JsonRpcResponse, _> = serde_json::from_str(&json);
    }
});