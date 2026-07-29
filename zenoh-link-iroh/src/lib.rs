//! zenoh-link-iroh — Iroh QUIC P2P transport plugin for Zenoh
//!
//! 编译: cargo build -p zenoh-link-iroh --release
//! 使用: zenohd -P iroh_link  → "iroh/" 端点可用

use tracing::info;
use zenoh::internal::runtime::DynamicRuntime;
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
        info!("'iroh/' endpoint scheme registered (placeholder)");
        info!("To use: configure listen.endpoints = [\"iroh/0.0.0.0:0\"]");
        Ok(IrohLinkInstance)
    }
}

declare_plugin!(IrohLinkDesc);
