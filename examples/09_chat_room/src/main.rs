//! # zenoh-chat-room — 基于 zenoh-link-iroh 的 P2P 控制台聊天室
//!
//! 运行: cargo run -- Alice my-room

mod chat;

use chat::{ChatManager, ChatMessage, User};
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use zenoh_link_state::iroh_integration::{ConnectionStatus, IrohTransportLink};

fn fake_node_id(name: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    name.hash(&mut h);
    format!("node_{:016x}", h.finish())
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

#[tokio::main]
async fn main() {
    // tracing 写入 stderr，避免抢 stdout
    tracing_subscriber::fmt().with_writer(io::stderr).init();

    let args: Vec<String> = std::env::args().collect();
    let user_name = args.get(1).map(|s| s.as_str()).unwrap_or("Anonymous");
    let room_name = args.get(2).map(|s| s.as_str()).unwrap_or("lobby");
    let node_id = fake_node_id(user_name);
    let manager = ChatManager::new(room_name, user_name, &node_id);

    let link = Arc::new(IrohTransportLink::new(node_id.clone(), "zenoh-link-iroh/1.0.0".into()));

    println!();
    println!("╔══════════════════════════════════════════╗");
    println!("║   Zenoh × Iroh  P2P 控制台聊天室         ║");
    println!("╠══════════════════════════════════════════╣");
    println!("║ 房间: {: <32}  ║", manager.room.lock().await.room_name);
    println!("║ 用户: {: <32}  ║", user_name);
    println!("║ ID:   {: <32}  ║", node_id);
    println!("║ 连接: {: <32}  ║", "Connected (模拟)");
    println!("╚══════════════════════════════════════════╝");
    println!();
    println!("  输入消息开始聊天。命令: /help, /users, /demo, /quit");
    println!();

    // 后台状态监控
    let monitor_link = Arc::clone(&link);
    let monitor_room = Arc::clone(&manager.room);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            let status = monitor_link.connection_status().await;
            if status != ConnectionStatus::Connected {
                let room = monitor_room.lock().await;
                println!("\n🔔 连接状态变化: {:?} | 在线: {} 人", status, room.member_count());
            }
        }
    });

    // 欢迎消息
    {
        let room = manager.room.lock().await;
        let msg = ChatMessage {
            sender: room.me.clone(), text: "👋 加入了聊天室".into(), timestamp_ms: now_ms(),
        };
        println!("{}", manager.format_message(&msg).await);
    }

    // 模拟在线用户
    for buddy in &["Bob", "Charlie", "Diana"] {
        let user = User { name: buddy.to_string(), node_id: fake_node_id(buddy) };
        let msg = ChatMessage {
            sender: user.clone(), text: "👋 加入了聊天室".into(), timestamp_ms: now_ms(),
        };
        manager.room.lock().await.upsert_member(user);
        println!("{}", manager.format_message(&msg).await);
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    // ── 主输入循环 (异步友好: spawn_blocking 读 stdin) ──
    loop {
        let input = tokio::task::spawn_blocking(|| {
            print!("> ");
            io::stdout().flush().ok();
            let mut line = String::new();
            match io::stdin().read_line(&mut line) {
                Ok(0) => None,
                Ok(_) => Some(line.trim().to_string()),
                Err(_) => None,
            }
        }).await.unwrap_or(None);

        match input {
            None => break,
            Some(ref s) if s.is_empty() => continue,
            Some(input) => {
                if input == "/quit" {
                    let room = manager.room.lock().await;
                    let msg = ChatMessage {
                        sender: room.me.clone(), text: "👋 离开了聊天室".into(), timestamp_ms: now_ms(),
                    };
                    println!("{}", manager.format_message(&msg).await);
                    break;
                }
                if input == "/help" {
                    println!("  命令: /help /users /demo /quit");
                    continue;
                }
                if input == "/users" {
                    let room = manager.room.lock().await;
                    println!("  📋 在线 {} 人:", room.member_count());
                    for (id, user) in &room.members {
                        let me = if *id == room.me.node_id { " (我)" } else { "" };
                        println!("     {} [{}]{}", user.name, id, me);
                    }
                    continue;
                }
                if input == "/demo" {
                    println!("\n  🎬 演示: 网络切换场景");
                    println!("  [1/4] 正在迁移...");
                    link.on_path_change(false).await;
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    println!("  [2/4] 断网期间消息排队");
                    link.on_path_change(true).await;
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    println!("  [3/4] 网络恢复，数据已发送 ✓");
                    println!("  [4/4] 状态: {:?}", link.connection_status().await);
                    println!();
                    continue;
                }
                if input.starts_with('/') {
                    println!("  未知命令: {}", input);
                    continue;
                }
                // 普通消息
                let mut room = manager.room.lock().await;
                let msg = ChatMessage {
                    sender: room.me.clone(), text: input, timestamp_ms: now_ms(),
                };
                room.record_message(msg.clone());
                println!("{}", manager.format_message(&msg).await);
            }
        }
    }

    link.disconnect().await;
    println!("\n👋 再见！");
}
