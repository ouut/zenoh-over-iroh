//! 示例: IrohTransportLink 集成使用
//!
//! 演示完整的 IrohTransportLink 用法:
//!   创建 → 写入 → 路径变化 → 恢复 → 超时 → 回调
//!
//! 运行: cargo run --example 05_integration_usage

use std::sync::Arc;
use std::time::Duration;
use zenoh_link_state::iroh_integration::{ConnectionStatus, IrohTransportLink};

#[tokio::main]
async fn main() {
    let link = Arc::new(IrohTransportLink::new(
        "demo_node_001".into(),
        "zenoh-link-iroh/1.0.0".into(),
    ));

    // ── 1. 初始状态 Connected ───────────────
    assert_eq!(link.connection_status().await, ConnectionStatus::Connected);
    println!("[link] Created: Connected ✓");

    // ── 2. 正常写入 ─────────────────────────
    assert!(link.write(b"hello_world".to_vec()).await.is_ok());
    println!("[write] Sent ✓");

    // ── 3. 路径失联 → Migrating ─────────────
    link.on_path_change(false).await;
    assert_eq!(link.connection_status().await, ConnectionStatus::Migrating);
    println!("[path]  → Migrating (path lost)");

    // ── 4. Migrating 期间写入排队 ───────────
    for i in 0..10 {
        assert!(link
            .write(format!("during_migration_{}", i).into_bytes())
            .await
            .is_ok());
    }
    println!("[write] 10 messages queued ✓");

    // ── 5. 路径恢复 → Connected ─────────────
    link.on_path_change(true).await;
    assert_eq!(link.connection_status().await, ConnectionStatus::Connected);
    println!("[path]  → Connected (path restored) ✓");

    // ── 6. 显式断开 ─────────────────────────
    link.disconnect().await;
    assert_eq!(
        link.connection_status().await,
        ConnectionStatus::Disconnected
    );
    assert!(link.write(b"after_close".to_vec()).await.is_err());
    println!("[link] Disconnected, writes rejected ✓");

    // ── 7. 超时回调演示 ─────────────────────
    println!();
    println!("[demo] Testing timeout callback...");
    let link2 = Arc::new(IrohTransportLink::new(
        "timeout_demo".into(),
        "test/1.0".into(),
    ));
    link2.on_path_change(false).await;

    let notified = Arc::new(tokio::sync::Notify::new());
    let n = Arc::clone(&notified);

    let _ticker = link2.start_ticker(move || {
        n.notify_one();
    });

    let result = tokio::time::timeout(Duration::from_millis(6000), notified.notified()).await;

    assert!(result.is_ok(), "Timeout callback should fire");
    assert_eq!(
        link2.connection_status().await,
        ConnectionStatus::Disconnected
    );
    println!("[timeout] Callback fired, link Disconnected ✓");

    println!();
    println!("=== 05_integration_usage: PASS ===");
}
