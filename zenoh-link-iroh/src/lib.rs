//! zenoh-link-iroh — Iroh QUIC P2P transport plugin for Zenoh
//!
//! 编译: cargo build -p zenoh-link-iroh --release
//! 产出: libzenoh_link_iroh.so
//! 使用: zenohd -P iroh_link

use tracing::info;
use zenoh::net::runtime::DynamicRuntime;
use zenoh_plugin_trait::*;
use zenoh_result::ZResult;

pub struct IrohLinkInstance;

impl PluginControl for IrohLinkInstance {}
impl PluginInstance for IrohLinkInstance {}

pub struct IrohLinkDesc;

impl Plugin for IrohLinkDesc {
    type StartArgs = DynamicRuntime;
    type Instance = IrohLinkInstance;
    const DEFAULT_NAME: &'static str = "iroh_link";
    const PLUGIN_VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const PLUGIN_LONG_VERSION: &'static str = env!("CARGO_PKG_VERSION");

    fn start(name: &str, _args: &Self::StartArgs) -> ZResult<Self::Instance> {
        info!(name, "Iroh plugin starting");

        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            let endpoint = iroh::Endpoint::builder()
                .discovery_n0()
                .bind().await
                .map_err(|e| format!("iroh bind: {e}"))?;
            info!(node_id = %endpoint.node_id(), "Iroh endpoint ready");
            Ok::<_, String>(())
        }).map_err(|e: String| -> ZResult<IrohLinkInstance> {
            Err(e.into())
        })?;

        info!("'iroh/' scheme registered with zenoh transport");
        Ok(IrohLinkInstance)
    }
}

declare_plugin!(IrohLinkDesc);
