//! P2P 聊天室 — Iroh QUIC 直连 + LinkStateMachine
//!
//! 运行:
//!   terminal A:  cargo run --release -- Alice
//!   terminal B:  cargo run --release -- Bob
//!   > /connect <对方的NodeID>
//!   然后双方即可收发消息（群聊 / 私信）

mod chat;

use chat::{ChatMessage, Commands};
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use zenoh_link_state::iroh_integration::{ConnectionStatus, IrohTransportLink};
use zenoh_link_state::link_state::WriteStatus;

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_writer(io::stderr).with_target(false).init();

    let name = std::env::args().nth(1).unwrap_or_else(|| "Anonymous".into());
    let room = std::env::args().nth(2).unwrap_or_else(|| "lobby".into());
    let my_id = format!("{}", std::process::id());

    // ── Iroh Endpoint ──
    info!(name, room, "Starting chat");
    let endpoint = iroh::Endpoint::builder()
        .discovery_n0()
        .relay_mode(iroh::RelayMode::Default)
        .alpns(vec![b"chat/1.0".to_vec()])
        .bind()
        .await?;

    let node_id = endpoint.node_id().to_string();
    info!(%node_id, "Iroh endpoint ready");

    let link = Arc::new(IrohTransportLink::new(node_id.clone(), "chat/1.0".into()));
    let peer_node: Arc<Mutex<Option<iroh::NodeId>>> = Arc::new(Mutex::new(None));

    // ── 接受传入连接 ──
    let accept_ep = endpoint.clone();
    let accept_link = Arc::clone(&link);
    let accept_peer = Arc::clone(&peer_node);
    let accept_name = name.clone();
    tokio::spawn(async move {
        while let Some(incoming) = accept_ep.accept().await {
            let link = Arc::clone(&accept_link);
            let peer = Arc::clone(&accept_peer);
            let name = accept_name.clone();
            tokio::spawn(async move {
                let conn = match incoming.await {
                    Ok(c) => c, Err(e) => { error!(%e); return; }
                };
                let remote = conn.remote_node_id().to_string();
                info!(%remote, "Incoming connection");
                link.on_path_change(true).await;

                // 发送欢迎消息
                let (mut send, _) = conn.open_bi().await.unwrap();
                let welcome = ChatMessage {
                    from: "system".into(), from_name: name,
                    text: format!("👋 加入了房间 {}", remote),
                    time: chrono::Local::now().format("%H:%M:%S").to_string(),
                    msg_type: "join".into(),
                };
                let _ = send.write_all(serde_json::to_string(&welcome)?.as_bytes()).await;
                let _ = send.finish();

                // 持续接收消息
                loop {
                    match conn.accept_bi().await {
                        Ok((_send, mut recv)) => {
                            let mut buf = vec![0u8; 65536];
                            match recv.read_to_end(&mut buf).await {
                                Ok(n) => {
                                    if let Ok(cm) = serde_json::from_slice::<ChatMessage>(&buf[..n]) {
                                        let ts = &cm.time;
                                        let tag = if cm.msg_type == "dm" { "🔒" } else { "📢" };
                                        println!("\r[{}] {} {}: {}", ts, tag, cm.from_name, cm.text);
                                        print!("> "); io::stdout().flush().ok();
                                    }
                                }
                                Err(e) => {
                                    warn!(%remote, %e, "Read error");
                                    link.on_path_change(false).await;
                                    break;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
                link.disconnect().await;
                info!(%remote, "Disconnected");
            });
        }
    });

    println!();
    println!("╔══════════════════════════════════════════╗");
    println!("║   P2P 聊天室 — Iroh QUIC                 ║");
    println!("╠══════════════════════════════════════════╣");
    println!("║ 用户: {:<32} ║", name);
    println!("║ 房间: {:<32} ║", room);
    println!("║ NodeID: {:<28} ║", &node_id[..28]);
    println!("╚══════════════════════════════════════════╝");
    println!("  /connect <NodeID>  连接到对方");
    println!("  /msg <user> <text> 私信");
    println!("  /users             在线用户");
    println!("  /quit              退出");
    println!();

    // ── 输入循环 ──
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    std::thread::spawn(move || {
        print!("> "); io::stdout().flush().ok();
        for line in io::stdin().lock().lines() {
            match line {
                Ok(s) => { let _ = tx.send(s); }
                Err(_) => break,
            }
        }
    });

    while let Some(input) = rx.recv().await {
        let input = input.trim().to_string();
        if input.is_empty() { print!("> "); io::stdout().flush().ok(); continue; }

        match input.as_str() {
            "/quit" => break,
            "/help" => println!("{}", Commands::HELP),
            "/users" => println!("  📋 在线: {} | 状态: {:?}", name, link.connection_status().await),
            s if s.starts_with("/connect ") => {
                let target = s.trim_start_matches("/connect ").trim();
                match target.parse::<iroh::NodeId>() {
                    Ok(peer_id) => {
                        *peer_node.lock().await = Some(peer_id);
                        debug!(%target, "Connecting");
                        match endpoint.connect(peer_id, b"chat/1.0".to_vec()).await {
                            Ok(conn) => {
                                info!(%target, "Connected!");
                                link.on_path_change(true).await;
                                println!("  ✅ 已连接 {}", &target[..16]);
                                let _ = link.write(b"connected".to_vec()).await;
                            }
                            Err(e) => {
                                warn!(%e, "Connect failed");
                                println!("  ❌ 连接失败: {}", e);
                            }
                        }
                    }
                    Err(_) => println!("  ❌ 无效 NodeID"),
                }
            }
            s if s.starts_with("/msg ") => {
                let rest = &s[5..];
                let text = if let Some(pos) = rest.find(' ') {
                    let peer = peer_node.lock().await;
                    let msg = ChatMessage {
                        from: node_id.clone(), from_name: name.clone(),
                        text: rest[pos+1..].to_string(),
                        time: chrono::Local::now().format("%H:%M:%S").to_string(),
                        msg_type: "dm".into(),
                    };
                    let payload = serde_json::to_string(&msg).unwrap();
                    let _ = link.write(payload.into_bytes()).await;
                    println!("[{}] 🔒 → {}: {}", msg.time, &rest[..pos], msg.text);
                    continue;
                };
                "私信格式: /msg <用户> <内容>"
            }
            text => {
                let msg = ChatMessage {
                    from: node_id.clone(), from_name: name.clone(),
                    text: text.to_string(),
                    time: chrono::Local::now().format("%H:%M:%S").to_string(),
                    msg_type: "room".into(),
                };
                let payload = serde_json::to_string(&msg).unwrap();
                match link.write(payload.into_bytes()).await {
                    Ok(()) => debug!("Message sent"),
                    Err(e) => warn!(%e, "Send failed"),
                }
                println!("[{}] 📢 {}: {}", msg.time, name, text);
            }
        }
        print!("> "); io::stdout().flush().ok();
    }

    endpoint.close().await;
    println!("\n👋 再见！");
    Ok(())
}
