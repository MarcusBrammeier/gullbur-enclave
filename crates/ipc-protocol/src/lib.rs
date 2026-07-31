pub mod eip6963;
pub mod messages;
pub mod token;

pub use eip6963::{Eip6963ProviderInfo, Eip6963Request};
pub use messages::{JsonRpcError, JsonRpcRequest, JsonRpcResponse, RpcError};
pub use token::AuthToken;

#[cfg(test)]
mod tests {
    use super::*;

    proptest::proptest! {
        #[test]
        fn proptest_rpc_error_roundtrip(code in -32768i32..32767, message in ".{0,100}") {
            let original = RpcError::new(code, &message);
            let json = serde_json::to_string(&original).expect("test invariant");
            let parsed: RpcError = serde_json::from_str(&json).expect("test invariant");
            assert_eq!(parsed.code, code);
            assert_eq!(parsed.message, message);
            assert!(parsed.data.is_none());
        }
    }

    proptest::proptest! {
        #[test]
        fn proptest_rpc_error_with_data_roundtrip(
            code in -32768i32..32767,
            message in ".{0,50}",
            data_val in ".{0,50}",
        ) {
            let data = serde_json::json!({"detail": data_val});
            let original = RpcError::with_data(code, &message, data.clone());
            let json = serde_json::to_string(&original).expect("test invariant");
            let parsed: RpcError = serde_json::from_str(&json).expect("test invariant");
            assert_eq!(parsed.code, code);
            assert_eq!(parsed.message, message);
            let parsed_data = parsed.data.expect("test invariant");
            assert_eq!(parsed_data["detail"], data["detail"]);
        }
    }
}
