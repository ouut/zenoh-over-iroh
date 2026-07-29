//! zenoh-link-iroh — Iroh QUIC P2P transport plugin for Zenoh.
//!
//! 编译为 cdylib (.so)，被 zenohd -P iroh_link 加载。
//! 加载后 "iroh/<node_id>" 成为有效 endpoint scheme。

use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use zenoh_plugin_trait::plugin::{Plugin, Runtime};

pub struct IrohPlugin;

impl Plugin for IrohPlugin {
    fn name(&self) -> &str { "iroh_link" }

    fn start(&mut self, runtime: &Runtime) -> Result<(), Box<dyn std::error::Error>> {
        let session = runtime.session();
        let config = runtime.config();

        info!("Iroh plugin starting");

        // 创建 Iroh Endpoint
        let endpoint = iroh::Endpoint::builder()
            .discovery_n0()
            .alpns(vec![b"zenoh-iroh/1.0".to_vec()])
            .bind_blocking()?;

        let node_id = endpoint.node_id().to_string();
        info!(%node_id, "Iroh endpoint ready");

        // 注册 LinkFactory
        // session.register_link_factory("iroh", LinkFactory(Arc::new(endpoint)))?;

        info!("Iroh plugin started");
        Ok(())
    }

    fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Iroh plugin stopping");
        Ok(())
    }
}

#[no_mangle]
pub extern "C" fn get_plugin_loader_version() -> u32 {
    zenoh_plugin_trait::PLUGIN_LOADER_VERSION
}

#[no_mangle]
pub extern "C" fn load_plugin() -> Box<dyn Plugin> {
    Box::new(IrohPlugin)
}
