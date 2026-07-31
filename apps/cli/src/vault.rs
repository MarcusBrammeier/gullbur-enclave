/// Headless vault helpers — creates a PluginHost with default plugins.
/// Only compiled when the `headless` feature is enabled.
#[cfg(feature = "headless")]
use std::sync::Arc;

/// Create a PluginHost with the default core plugins registered.
#[cfg(feature = "headless")]
pub async fn create_plugin_host() -> Result<vault_core::host::PluginHost, Box<dyn std::error::Error>>
{
    let mut host = vault_core::host::PluginHost::new();

    // Register default plugins
    host.register(Arc::new(plugin_btc::BtcPlugin::new(None)));
    host.register(Arc::new(plugin_evm::EvmPlugin::new()));
    host.register(Arc::new(plugin_xmr::XmrPlugin::new()));
    host.register(Arc::new(plugin_ltc::LtcPlugin::new()));

    Ok(host)
}
