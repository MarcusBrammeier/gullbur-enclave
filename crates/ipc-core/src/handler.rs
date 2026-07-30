use std::collections::HashMap;
use std::sync::Arc;

use ipc_protocol::{JsonRpcRequest, JsonRpcResponse, JsonRpcError, RpcError};

pub type HandlerFn = Arc<
    dyn Fn(serde_json::Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value, RpcError>> + Send>>
        + Send + Sync,
>;

pub enum DispatchResult {
    Success(JsonRpcResponse),
    Error(JsonRpcError),
}

pub struct MessageHandler {
    handlers: HashMap<String, HandlerFn>,
}

impl MessageHandler {
    pub fn new() -> Self { Self { handlers: HashMap::new() } }

    pub fn register<F, Fut>(&mut self, method: &str, handler: F)
    where
        F: Fn(serde_json::Value) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<serde_json::Value, RpcError>> + Send + 'static,
    {
        self.handlers.insert(method.to_string(), Arc::new(move |params| Box::pin(handler(params))));
    }

    pub async fn dispatch(&self, request: JsonRpcRequest) -> DispatchResult {
        let id = request.id;
        match self.handlers.get(&request.method) {
            Some(handler) => match handler(request.params.unwrap_or(serde_json::Value::Null)).await {
                Ok(result) => DispatchResult::Success(JsonRpcResponse::new(result, id)),
                Err(e) => DispatchResult::Error(JsonRpcError::new(e, id)),
            },
            None => DispatchResult::Error(JsonRpcError::new(
                RpcError::method_not_found(), id,
            )),
        }
    }
}

impl Default for MessageHandler {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ipc_protocol::JsonRpcRequest;

    #[tokio::test]
    async fn test_handler_new_and_method_not_found() {
        let handler = MessageHandler::new();
        let req = JsonRpcRequest::new("unknown_method", None, 1);
        match handler.dispatch(req).await {
            DispatchResult::Error(err) => {
                assert_eq!(err.error.code, -32601);
                assert_eq!(err.id, 1);
            }
            _ => panic!("Expected Error for unknown method"),
        }
    }

    #[tokio::test]
    async fn test_handler_register_and_dispatch() {
        let mut handler = MessageHandler::new();
        handler.register("echo", |params| async move { Ok(params) });
        let req = JsonRpcRequest::new(
            "echo",
            Some(serde_json::json!({"hello": "world"})),
            42,
        );
        match handler.dispatch(req).await {
            DispatchResult::Success(resp) => {
                assert_eq!(resp.id, 42);
                assert_eq!(resp.result, serde_json::json!({"hello": "world"}));
            }
            _ => panic!("Expected Success"),
        }
    }

    #[test]
    fn test_handler_new_is_empty() {
        let _handler = MessageHandler::new();
        // Verifies construction without error
    }
}