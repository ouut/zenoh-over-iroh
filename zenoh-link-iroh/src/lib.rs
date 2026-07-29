//! zenoh-link-iroh plugin
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
        info!(name, "Iroh plugin started");
        Ok(IrohLinkInstance)
    }
}
declare_plugin!(IrohLinkDesc);
