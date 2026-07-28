//! # zenoh-chat-room — 基于 zenoh-link-iroh 的 P2P 控制台聊天室
//!
//! ## 功能
//!
//! - 通过 Iroh P2P 网络直连通信（打洞优先，Relay 保底）
//! - 基于 zenoh pub/sub 的消息分发
//! - 使用 LinkStateMachine 处理网络切换，对话不中断
//! - 命令: /help, /users, /quit
//!
//! ## 架构
//!
//! ```text
//! 用户 A (终端)               用户 B (终端)
//!     │                            │
//!     ▼                            ▼
//! ChatRoom + LinkStateMachine   ChatRoom + LinkStateMachine
//!     │                            │
//!     ▼                            ▼
//! zenoh Session (pub/sub)  ←→  zenoh Session (pub/sub)
//!     │                            │
//!     ▼                            ▼
//! Iroh Endpoint  ←── P2P/Relay ──→  Iroh Endpoint
//! ```
//!
//! ## 快速开始
//!
//! ```bash
//! # 终端 1 (用户 Alice)
//! cd examples/09_chat_room
//! cargo run -- Alice my-room
//!
//! # 终端 2 (用户 Bob)
//! cd examples/09_chat_room
//! cargo run -- Bob my-room
//! ```
//!
//! ## 部署到生产环境
//!
//! 参见本目录下的 `README.md`。

mod chat;

use chat::{ChatManager, ChatMessage, User};
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use zenoh_link_state::iroh_integration::{ConnectionStatus, IrohTransportLink};

/// 生成假的 NodeID（实际部署时由 Iroh 公钥生成）。
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
    tracing_subscriber::fmt::init();

    let args: Vec<String> = std::env::args().collect();
    let user_name = args.get(1).map(|s| s.as_str()).unwrap_or("Anonymous");
    let room_name = args.get(2).map(|s| s.as_str()).unwrap_or("lobby");

    let node_id = fake_node_id(user_name);
    let manager = ChatManager::new(room_name, user_name, &node_id);

    // ════════════════════════════════════════════════════════
    //  模拟 Iroh + Zenoh 连接（实际部署见 README.md）
    // ════════════════════════════════════════════════════════

    let link = Arc::new(IrohTransportLink::new(
        node_id.clone(),
        "zenoh-link-iroh/1.0.0".into(),
    ));

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
    println!("║ 连接: {: <32}  ║", "Connected (模拟)");
    println!("╚══════════════════════════════════════════╝");
    println!();
    println!("  输入消息开始聊天。命令: /help, /users, /demo, /quit");
    println!();

    // ── 演示命令: /demo ────────────────────────
    // 展示状态机在网络波动下的行为

    // ── 后台状态监控 ───────────────────────────
    let monitor_link = Arc::clone(&link);
    let monitor_room = Arc::clone(&manager.room);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            let status = monitor_link.connection_status().await;
            let room = monitor_room.lock().await;
            if status != ConnectionStatus::Connected {
                println!(
                    "\n🔔 [系统] 连接状态变化: {:?} | 在线: {} 人",
                    status,
                    room.member_count()
                );
            }
        }
    });

    // ── 运行演示序列 ───────────────────────────
    {
        let room = manager.room.lock().await;
        let msg = ChatMessage {
            sender: room.me.clone(),
            text: "👋 加入了聊天室".into(),
            timestamp_ms: now_ms(),
        };
        println!("{}", manager.format_message(&msg).await);
    }

    // 模拟一些系统消息
    for buddy in &["Bob", "Charlie", "Diana"] {
        let user = User {
            name: buddy.to_string(),
            node_id: fake_node_id(buddy),
        };
        let msg = ChatMessage {
            sender: user.clone(),
            text: format!("👋 加入了聊天室"),
            timestamp_ms: now_ms(),
        };
        manager.room.lock().await.upsert_member(user);
        println!("{}", manager.format_message(&msg).await);
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    // ── 主输入循环 ─────────────────────────────
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("> ");
        stdout.flush().unwrap();

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {
                let input = line.trim().to_string();
                if input.is_empty() {
                    continue;
                }

                // 命令处理
                if input.starts_with('/') {
                    match input.as_str() {
                        "/help" => {
                            println!("  命令:");
                            println!("    /help   - 显示帮助");
                            println!("    /users  - 查看在线用户");
                            println!("    /demo   - 演示状态机断线恢复");
                            println!("    /quit   - 退出聊天室");
                        }
                        "/users" => {
                            let room = manager.room.lock().await;
                            println!("  📋 在线用户 ({} 人):", room.member_count());
                            for (id, user) in &room.members {
                                let marker = if *id == room.me.node_id { " (我)" } else { "" };
                                println!("     {} [{}]{}", user.name, id, marker);
                            }
                        }
                        "/demo" => {
                            println!("\n  🎬 演示: 网络切换场景");
                            println!("  ────────────────────────");

                            // 模拟断网
                            println!("  [1/4] 正在迁移 (网络切换中)...");
                            link.on_path_change(false).await;
                            tokio::time::sleep(Duration::from_secs(1)).await;

                            // 断网期间"发送"消息（被状态机排队）
                            {
                                let room = manager.room.lock().await;
                                let msg = ChatMessage {
                                    sender: room.me.clone(),
                                    text: "这条消息在断网期间排队，恢复后发送".into(),
                                    timestamp_ms: now_ms(),
                                };
                                println!(
                                    "  [2/4] 断网期间排队: {}",
                                    manager.format_message(&msg).await
                                );
                            }

                            // 恢复
                            link.on_path_change(true).await;
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            println!("  [3/4] 网络恢复，排队数据已发送 ✓");

                            // 显示状态
                            let status = link.connection_status().await;
                            println!("  [4/4] 连接状态: {:?}", status);
                            println!();
                        }
                        "/quit" => {
                            let room = manager.room.lock().await;
                            let msg = ChatMessage {
                                sender: room.me.clone(),
                                text: "👋 离开了聊天室".into(),
                                timestamp_ms: now_ms(),
                            };
                            println!("{}", manager.format_message(&msg).await);
                            break;
                        }
                        _ => {
                            println!("  未知命令，输入 /help 查看帮助");
                        }
                    }
                } else {
                    // 普通消息
                    let mut room = manager.room.lock().await;
                    let msg = ChatMessage {
                        sender: room.me.clone(),
                        text: input,
                        timestamp_ms: now_ms(),
                    };
                    room.record_message(msg.clone());
                    println!("{}", manager.format_message(&msg).await);
                }
            }
            Err(e) => {
                eprintln!("读取错误: {}", e);
                break;
            }
        }
    }

    // ── 清理 ────────────────────────────────────
    link.disconnect().await;
    println!("\n👋 再见！");
}
