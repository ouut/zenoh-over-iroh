//! zenoh-link-iroh — Iroh QUIC transport for Zenoh
//!
//! ## 两个使用方式
//!
//! ### 1. 插件 (cdylib) — zenohd -P iroh_link
//!   zenohd 加载后创建 Iroh Endpoint。
//!   注意：zenoh 1.9 不支持运行时注册新传输层 scheme，
//!   因此 "iroh/" 配置尚不能直接使用。
//!   如需通过 Iroh 传输 Zenoh 消息，请使用方式 2。
//!
//! ### 2. 库 (library) — cargo add zenoh-link-iroh
//!   和 zenoh-link-tcp 一样作为编译时传输层。
//!   需要 fork zenoh 并加入本 crate 依赖。
//!   详见 `doc/生产部署检查清单.md`。

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
        info!(name, "Iroh plugin loading...");

        // 在 zenohd 的 tokio runtime 上创建 Iroh Endpoint
        match std::thread::spawn(move || {
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                iroh::Endpoint::builder()
                    .discovery_n0()
                    .relay_mode(iroh::RelayMode::Default)
                    .alpns(vec![b"zenoh-iroh/1.0".to_vec()])
                    .bind().await
            })
        }).join() {
            Ok(Ok(ep)) => {
                let nid = ep.node_id().to_string();
                info!(node_id = %nid, "Iroh Endpoint created");
                info!("Note: 'iroh/' scheme requires compile-time integration");
                info!("See doc/生产部署检查清单.md for production deployment");
            }
            Ok(Err(e)) => info!("Iroh Endpoint creation deferred: {e}"),
            Err(_) => info!("Iroh Endpoint creation deferred (no tokio runtime)"),
        }

        Ok(IrohLinkInstance)
    }
}

declare_plugin!(IrohLinkDesc);
