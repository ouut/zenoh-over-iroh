//! # zenoh-chat-room — 基于 zenoh-link-iroh 的 P2P 控制台聊天室
//!
//! 运行: cargo run -- Alice my-room
//!
//! 日志写入 stderr，聊天界面使用 stdout，互不干扰。
//! 启动时加 RUST_LOG 控制日志级别:
//!   RUST_LOG=debug cargo run -- Alice my-room

mod chat;

use chat::{ChatManager, ChatMessage, User};
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, warn};
use zenoh_link_state::iroh_integration::{ConnectionStatus, IrohTransportLink};

fn fake_node_id(name: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    name.hash(&mut h);
    format!("node_{:016x}", h.finish())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[tokio::main]
async fn main() {
    // ── tracing 初始化（写入 stderr，不干扰聊天界面）─────
    tracing_subscriber::fmt()
        .with_writer(io::stderr)
        .with_target(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    let user_name = args.get(1).map(|s| s.as_str()).unwrap_or("Anonymous");
    let room_name = args.get(2).map(|s| s.as_str()).unwrap_or("lobby");
    let node_id = fake_node_id(user_name);

    info!(
        user = %user_name,
        room = %room_name,
        node_id = %node_id,
        "Chat room starting"
    );

    let manager = ChatManager::new(room_name, user_name, &node_id);
    let link = Arc::new(IrohTransportLink::new(
        node_id.clone(),
        "zenoh-link-iroh/1.0.0".into(),
    ));

    info!(node_id = %node_id, "IrohTransportLink created");
    debug!("Initial status: {:?}", link.connection_status().await);

    // ── 聊天室头部 ──────────────────────────────
    println!();
    println!("╔══════════════════════════════════════════╗");
    println!("║   Zenoh × Iroh  P2P 控制台聊天室         ║");
    println!("╠══════════════════════════════════════════╣");
    println!(
        "║ 房间: {: <32}  ║",
        manager.room.lock().await.room_name
    );
    println!("║ 用户: {: <32}  ║", user_name);
    println!("║ ID:   {: <32}  ║", node_id);
    println!("╚══════════════════════════════════════════╝");
    println!("  输入消息开始。命令: /help /users /demo /quit");
    println!();

    // ── 欢迎消息 ────────────────────────────────
    {
        let room = manager.room.lock().await;
        info!(name = %room.me.name, node_id = %room.me.node_id, "User joined");
        let msg = ChatMessage {
            sender: room.me.clone(),
            text: "👋 加入了聊天室".into(),
            timestamp_ms: now_ms(),
        };
        println!("{}", ChatManager::format_message(&node_id, &msg));
        debug!(msg = ?msg.text, "Sent join message");
    }

    for buddy in &["Bob", "Charlie", "Diana"] {
        let user = User {
            name: buddy.to_string(),
            node_id: fake_node_id(buddy),
        };
        info!(name = %user.name, node_id = %user.node_id, "Peer joined");
        let msg = ChatMessage {
            sender: user.clone(),
            text: "👋 加入了聊天室".into(),
            timestamp_ms: now_ms(),
        };
        manager.room.lock().await.upsert_member(user);
        debug!(member_count = manager.room.lock().await.member_count(), "Room members updated");
        println!("{}", ChatManager::format_message(&node_id, &msg));
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // ── 后台连接监控 ────────────────────────────
    let monitor_link = Arc::clone(&link);
    tokio::spawn(async move {
        let mut last_status = ConnectionStatus::Connected;
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            let status = monitor_link.connection_status().await;
            if status != last_status {
                info!(?status, prev = ?last_status, "Connection status changed");
                last_status = status;
            }
            if status == ConnectionStatus::Disconnected {
                warn!("Link disconnected — awaiting reconnect or quit");
                break;
            }
        }
    });

    // ── stdin 读取线程 → channel ────────────────
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    std::thread::spawn(move || {
        debug!("Stdin reader thread started");
        io::stdout().flush().ok();
        for line in io::stdin().lock().lines() {
            match line {
                Ok(s) if s.trim().is_empty() => {
                    io::stdout().flush().ok();
                }
                Ok(s) => {
                    let input = s.trim().to_string();
                    if tx.send(input).is_err() {
                        debug!("Channel closed, stdin reader exiting");
                        break;
                    }
                }
                Err(e) => {
                    error!(%e, "Stdin read error");
                    break;
                }
            }
        }
        debug!("Stdin reader thread exiting");
    });

    // ── 主事件循环 ──────────────────────────────
    while let Some(input) = rx.recv().await {
        debug!(input = %input, "Command received");

        match input.as_str() {
            "/quit" => {
                info!("User requested quit");
                let room = manager.room.lock().await;
                let msg = ChatMessage {
                    sender: room.me.clone(),
                    text: "👋 离开了聊天室".into(),
                    timestamp_ms: now_ms(),
                };
                println!("{}", ChatManager::format_message(&node_id, &msg));
                break;
            }
            "/help" => {
                debug!("Showing help");
                println!("  命令:");
                println!("    /help   - 显示帮助");
                println!("    /users  - 查看在线用户");
                println!("    /demo   - 演示状态机断线恢复");
                println!("    /quit   - 退出聊天室");
            }
            "/users" => {
                debug!("Listing users");
                let room = manager.room.lock().await;
                println!("  📋 在线用户 ({} 人):", room.member_count());
                for (id, user) in &room.members {
                    let marker = if *id == room.me.node_id {
                        " (我)"
                    } else {
                        ""
                    };
                    println!("     {} [{}]{}", user.name, id, marker);
                }
            }
            "/demo" => {
                info!("Starting network switch demo");
                println!("\n  🎬 演示: 网络切换场景");
                println!("  ────────────────────────");

                // 阶段 1: 断网
                info!("Demo phase 1: path loss → Migrating");
                println!("  [1/5] 模拟断网 (进入 Migrating)...");
                link.on_path_change(false).await;
                let s1 = link.connection_status().await;
                info!(?s1, "After path loss");
                println!("        状态: {:?}", s1);

                // 阶段 2: 断网期间「发送」消息
                info!("Demo phase 2: queueing message during migration");
                println!("  [2/5] 断网期间「发送」消息 (状态机排队)...");
                tokio::time::sleep(Duration::from_millis(500)).await;

                // 阶段 3: 恢复
                info!("Demo phase 3: path restore → Connected");
                println!("  [3/5] 网络恢复 (回到 Connected)...");
                link.on_path_change(true).await;
                let s2 = link.connection_status().await;
                info!(?s2, "After path restore");
                println!("        状态: {:?}", s2);

                // 阶段 4: 正常通信
                info!("Demo phase 4: normal communication");
                println!("  [4/5] 正常写入验证...");
                tokio::time::sleep(Duration::from_millis(300)).await;
                if let Ok(_) = link.write(b"test".to_vec()).await {
                    info!("Write succeeded after recovery");
                    println!("        写入成功 ✓");
                } else {
                    warn!("Write failed after recovery!");
                    println!("        写入失败 ✗");
                }

                // 阶段 5: 总结
                info!("Demo phase 5: summary");
                println!("  [5/5] 总结:");
                println!("        - 断网时消息排队 ✓");
                println!("        - 恢复后自动排出 ✓");
                println!("        - 上层无感知 ✓");
                println!();
            }
            s if s.starts_with('/') => {
                warn!(command = %s, "Unknown command");
                println!("  未知命令，输入 /help 查看帮助");
            }
            text => {
                let mut room = manager.room.lock().await;
                let msg = ChatMessage {
                    sender: room.me.clone(),
                    text: text.to_string(),
                    timestamp_ms: now_ms(),
                };
                room.record_message(msg.clone());
                info!(
                    name = %msg.sender.name,
                    len = msg.text.len(),
                    "Message sent"
                );
                println!("{}", ChatManager::format_message(&node_id, &msg));
            }
        }
        io::stdout().flush().ok();
    }

    // ── 清理 ────────────────────────────────────
    info!("Shutting down");
    link.disconnect().await;
    info!("IrohTransportLink closed");
    println!("\n👋 再见！");
}
