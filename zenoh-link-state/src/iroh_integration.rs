//! # zenoh-link-iroh 插件集成层
//!
//! 本模块定义 `LinkStateMachine` 如何集成到 zenoh 的 transport 层。
//!
//! ## 架构
//!
//! ```text
//! zenoh_transport::LinkUnicastTrait (zenoh 定义的对外接口)
//!         │
//!         ▼
//! IrohTransportLink (本模块)
//!         │
//!    ┌────┴────┐
//!    ▼         ▼
//! LinkStateMachine   iroh::Endpoint (QUIC 连接管理)
//! (状态过滤)          (实际网络 IO)
//! ```
//!
//! ## 关键设计决策
//!
//! 1. `LinkStateMachine` 是非侵入式的：位于 zenoh trait 和实际 IO 之间
//! 2. 状态机不修改 zenoh 的公开 trait 签名
//! 3. `Migrating` 态期间 zenoh 侧的 `write()` 调用返回成功（数据排队）
//! 4. 超时后由状态机的 `tick()` 驱动上抛连接失效事件

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing;

// 从主 crate 导入状态机（实际使用时路径为 zenoh_link_state::link_state）
// use crate::link_state::{LinkStateMachine, LinkEvent, WriteStatus, LinkError};

// ═══════════════════════════════════════════════════════════════
//  IrohTransportLink — 集成状态机的 LinkUnicast 实现
// ═══════════════════════════════════════════════════════════════

/// 连接状态（对外暴露的简化视图）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// 连接正常，数据可发送。
    Connected,
    /// 路径迁移中，数据排队但连接未断。
    Migrating,
    /// 连接已断开。
    Disconnected,
}

/// Iroh Transport Link — 封装 LinkStateMachine + Iroh 连接的桥梁。
///
/// 此结构体实现了 `zenoh_transport::LinkUnicastTrait` 所需的方法，
/// 在内部委托给 `LinkStateMachine` 进行状态过滤。
pub struct IrohTransportLink {
    /// 三态状态机（核心设计 §1.4）。
    state_machine: Arc<Mutex<crate::link_state::LinkStateMachine>>,

    /// Iroh NodeID（对端标识）。
    node_id: String,

    /// ALPN 协议标识。
    alpn: String,

    /// 标记是否已关闭。
    closed: Arc<Mutex<bool>>,
}

impl IrohTransportLink {
    /// 创建新的 IrohTransportLink。
    ///
    /// 初始状态为 Connected。
    pub fn new(node_id: String, alpn: String) -> Self {
        tracing::info!(
            node_id = %node_id,
            alpn = %alpn,
            "IrohTransportLink created"
        );

        Self {
            state_machine: Arc::new(Mutex::new(
                crate::link_state::LinkStateMachine::new(),
            )),
            node_id,
            alpn,
            closed: Arc::new(Mutex::new(false)),
        }
    }

    /// 获取当前连接状态（对外视图）。
    pub async fn connection_status(&self) -> ConnectionStatus {
        let sm = self.state_machine.lock().await;
        if sm.is_connected() {
            ConnectionStatus::Connected
        } else if sm.is_migrating() {
            ConnectionStatus::Migrating
        } else {
            ConnectionStatus::Disconnected
        }
    }

    /// 写入数据。
    ///
    /// 对应 `zenoh_transport::LinkUnicastTrait::write()`。
    ///
    /// - Connected: 直接写入 Iroh SendStream
    /// - Migrating: 数据排队（返回成功，不报错）
    /// - Disconnected: 返回错误
    pub async fn write(&self, data: Vec<u8>) -> Result<(), String> {
        let mut sm = self.state_machine.lock().await;

        match sm.write(data) {
            Ok(crate::link_state::WriteStatus::Sent) => {
                tracing::trace!(len = sm.queue_len(), "Data sent immediately");
                // TODO: 实际调用 iroh::SendStream::write_all()
                Ok(())
            }
            Ok(crate::link_state::WriteStatus::Queued) => {
                tracing::debug!(queue_depth = sm.queue_len(), "Data queued during migration");
                Ok(()) // 关键：Migrating 态不报错
            }
            Ok(crate::link_state::WriteStatus::Backpressure) => {
                tracing::warn!("Write rejected: backpressure (queue full)");
                Err("backpressure: queue full".into())
            }
            Err(crate::link_state::LinkError::Disconnected) => {
                tracing::error!("Write rejected: link disconnected");
                Err("link disconnected".into())
            }
        }
    }

    /// 读取数据（非阻塞检查）。
    ///
    /// 对应 `zenoh_transport::LinkUnicastTrait::read()`。
    pub async fn can_read(&self) -> Result<(), String> {
        let sm = self.state_machine.lock().await;
        match sm.read() {
            Ok(()) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// QUIC 路径变化通知。
    ///
    /// 由 Iroh Endpoint 回调触发：
    /// - `path_alive = false` → Connected → Migrating
    /// - `path_alive = true`  → Migrating → Connected（并排出排队数据）
    ///
    /// 注意：此方法返回的事件仅用于 tracing，不参与 Zenoh 重连决策。
    pub async fn on_path_change(&self, path_alive: bool) {
        let mut sm = self.state_machine.lock().await;

        if let Some(event) = sm.on_path_change(path_alive) {
            match event {
                crate::link_state::LinkEvent::PathMigrated => {
                    tracing::info_span!("link.path_migrated", node_id = %self.node_id)
                        .in_scope(|| tracing::info!("Entering Migrating state"));
                }
                crate::link_state::LinkEvent::PathRestored => {
                    let queue_len = sm.queue_len();
                    tracing::info_span!("link.path_restored", node_id = %self.node_id)
                        .in_scope(|| tracing::info!(queue_len, "Returning to Connected"));

                    // 恢复后排出排队数据，重新发送
                    let drained: Vec<Vec<u8>> = sm.drain_queue().into_iter().collect();
                    if !drained.is_empty() {
                        tracing::info!(
                            drained_count = drained.len(),
                            "Flushing queued data after path restoration"
                        );
                        // TODO: 实际通过 Iroh SendStream 批量发送 drained 数据
                    }
                }
                _ => {}
            }
        }
    }

    /// 定时轮询（驱动超时检测）。
    ///
    /// 应由 tokio 定时器每 100ms 调用一次。
    /// 若返回 `Some(MigrationTimeout)`，调用方应通知 Zenoh 上层触发重连。
    pub async fn tick(&self) -> Option<crate::link_state::LinkEvent> {
        let mut sm = self.state_machine.lock().await;

        if let Some(event) = sm.tick() {
            tracing::warn!(
                node_id = %self.node_id,
                ?event,
                "State machine timeout event"
            );
            return Some(event);
        }
        None
    }

    /// 显式断开连接。
    pub async fn disconnect(&self) {
        let is_closed = { *self.closed.lock().await };
        if is_closed {
            return;
        }

        let mut sm = self.state_machine.lock().await;
        sm.disconnect();
        *self.closed.lock().await = true;

        tracing::info!(node_id = %self.node_id, "IrohTransportLink disconnected");
    }

    /// 启动状态机轮询任务。
    ///
    /// 返回一个 tokio JoinHandle，在后台每 100ms 调用 `tick()`。
    /// 当检测到 `MigrationTimeout` 时，调用 `on_timeout` 回调。
    pub fn start_ticker<F>(self: &Arc<Self>, on_timeout: F) -> tokio::task::JoinHandle<()>
    where
        F: Fn() + Send + 'static,
    {
        let link = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;

                if *link.closed.lock().await {
                    break;
                }

                if let Some(event) = link.tick().await {
                    if matches!(event, crate::link_state::LinkEvent::MigrationTimeout) {
                        tracing::error!(
                            node_id = %link.node_id,
                            "Migration timeout — triggering reconnect"
                        );
                        on_timeout();
                        break; // 超时后停止轮询，等待上层重连
                    }
                }
            }
        })
    }
}

// ═══════════════════════════════════════════════════════════════
//  IrohLinkManager — LinkManagerUnicastTrait 实现骨架
// ═══════════════════════════════════════════════════════════════

/// Iroh Link Manager — 管理 Iroh Endpoint 生命周期，创建连接。
///
/// 对应 `zenoh_transport::LinkManagerUnicastTrait`。
pub struct IrohLinkManager {
    /// Iroh Endpoint（实际部署时由 iroh-net crate 提供）
    /// iroh_endpoint: iroh::Endpoint,
    _placeholder: (),
}

impl IrohLinkManager {
    pub fn new() -> Self {
        Self { _placeholder: () }
    }

    /// 拨号到远程节点。
    ///
    /// 对应 `zenoh_transport::LinkManagerUnicastTrait::new_link()`。
    ///
    /// Locator 格式：`iroh/<node_id>`
    pub async fn dial(&self, locator: &str) -> Result<Arc<IrohTransportLink>, String> {
        // 解析 iroh/<node_id>
        let node_id = locator
            .strip_prefix("iroh/")
            .ok_or_else(|| format!("Invalid iroh locator: {}", locator))?
            .to_string();

        tracing::info!(%node_id, "Dialing Iroh link");

        // TODO: 实际调用 iroh::Endpoint::connect(node_id, ALPN)
        let link = Arc::new(IrohTransportLink::new(
            node_id,
            "zenoh-link-iroh/1.0.0".into(),
        ));

        Ok(link)
    }

    /// 监听传入的 Iroh 连接。
    ///
    /// 后台持续 accept，每收到新连接创建一个 IrohTransportLink。
    pub async fn listen(&self, _bind_addr: &str) -> Result<(), String> {
        // TODO: 实际调用 iroh::Endpoint::accept()
        tracing::info!("Listening for Iroh connections...");
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════
//  集成示例
// ═══════════════════════════════════════════════════════════════

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// 测试完整的集成流程：拨号 → 路径变化 → 恢复 → 断开
    #[tokio::test]
    async fn test_full_integration_lifecycle() {
        let link = Arc::new(IrohTransportLink::new("test_node_001".into(), "test/1.0".into()));

        // 1. 初始状态：Connected
        assert_eq!(
            link.connection_status().await,
            ConnectionStatus::Connected
        );

        // 2. 正常写入
        assert!(link.write(b"hello".to_vec()).await.is_ok());

        // 3. 路径失联 → Migrating
        link.on_path_change(false).await;
        assert_eq!(
            link.connection_status().await,
            ConnectionStatus::Migrating
        );

        // 4. Migrating 期间写入排队（不报错）
        assert!(link.write(b"world".to_vec()).await.is_ok());

        // 5. 路径恢复 → Connected
        link.on_path_change(true).await;
        assert_eq!(
            link.connection_status().await,
            ConnectionStatus::Connected
        );

        // 6. 显式断开
        link.disconnect().await;
        assert!(link.write(b"after_close".to_vec()).await.is_err());
    }

    /// 测试超时触发 on_timeout 回调
    #[tokio::test]
    async fn test_timeout_triggers_callback() {
        let link = Arc::new(IrohTransportLink::new("timeout_test".into(), "test/1.0".into()));

        // 进入 Migrating
        link.on_path_change(false).await;

        let called = Arc::new(tokio::sync::Notify::new());
        let called_clone = Arc::clone(&called);

        // 启动 ticker，超时时 notify
        let _ticker = link.start_ticker(move || {
            called_clone.notify_one();
        });

        // 等待超时（最多 6s）
        let result = tokio::time::timeout(
            Duration::from_millis(6000),
            called.notified(),
        )
        .await;

        assert!(result.is_ok(), "on_timeout should have been called within 6s");
        assert_eq!(
            link.connection_status().await,
            ConnectionStatus::Disconnected
        );
    }
}
