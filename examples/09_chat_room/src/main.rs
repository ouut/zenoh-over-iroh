//! # zenoh-chat-room — 基于真实 Iroh P2P 的控制台聊天室
//!
//! 终端1: cargo run -- Alice
//!   输出:  NodeID: 2lue3...
//!   输入:  /connect <对方NodeID>
//!
//! 终端2: cargo run -- Bob
//!   输入:  /connect <Alice的NodeID>

mod chat;
mod iroh_chat;

use chat::ChatManager;
use iroh_chat::{IrohChatTransport, WireMessage};
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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

    // ── 真实 Iroh 传输 ────────────────────
    info!("Starting Iroh endpoint...");
    let mut transport = IrohChatTransport::new("0.0.0.0:0", None, None).await?;
    let node_id = transport.node_id.clone();

    println!();
    println!("╔══════════════════════════════════════════════════╗");
    println!("║   Zenoh × Iroh  P2P 控制台聊天室 (真实网络)     ║");
    println!("╠══════════════════════════════════════════════════╣");
    println!("║ 用户:   {: <38}  ║", user_name);
    println!("║ NodeID: {: <38}  ║", node_id);
    println!("╚══════════════════════════════════════════════════╝");
    println!("  输入 /connect <对方NodeID> 连接后开始聊天");
    println!();

    let peer_id: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    // ── stdin 线程 → channel ────
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    std::thread::spawn(move || {
        print!("> ");
        io::stdout().flush().ok();
        for line in io::stdin().lock().lines() {
            match line {
                Ok(s) if s.trim().is_empty() => {
                    print!("> ");
                    io::stdout().flush().ok();
                }
                Ok(s) => {
                    if tx.send(s.trim().to_string()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // ── 主循环 ──────────────────────
    loop {
        tokio::select! {
            // 用户输入
            input = rx.recv() => {
                let Some(input) = input else { break };
                debug!(%input, "Command");

                match input.as_str() {
                    "/quit" => { info!("Quit"); break; }
                    "/help" => println!("  /connect <NodeID>  /help  /quit"),
                    s if s.starts_with("/connect ") => {
                        let remote = s.trim_start_matches("/connect ").trim();
                        info!(%remote, "Connecting");
                        *peer_id.lock().await = Some(remote.to_string());
                        match transport.connect(remote).await {
                            Ok(()) => {
                                println!("  ✅ 已连接 {}", remote);
                                let msg = WireMessage {
                                    sender_name: user_name.into(),
                                    sender_node_id: node_id.clone(),
                                    text: "👋 加入了聊天室".into(),
                                    timestamp_ms: now_ms(),
                                    msg_type: "join".into(),
                                };
                                let _ = transport.send(remote, msg).await;
                            }
                            Err(e) => println!("  ❌ 连接失败: {}", e),
                        }
                    }
                    text => {
                        let peer = peer_id.lock().await.clone();
                        match peer {
                            Some(ref pid) => {
                                let msg = WireMessage {
                                    sender_name: user_name.into(),
                                    sender_node_id: node_id.clone(),
                                    text: text.to_string(),
                                    timestamp_ms: now_ms(),
                                    msg_type: "msg".into(),
                                };
                                match transport.send(pid, msg).await {
                                    Ok(()) => println!("[{}] 👤 我: {}", now_ms() % 100000, text),
                                    Err(e) => println!("  ❌ 发送失败: {}", e),
                                }
                            }
                            None => {
                                println!("  请先 /connect <对方NodeID>");
                                println!("  你的 NodeID: {}", node_id);
                            }
                        }
                    }
                }
                print!("> "); io::stdout().flush().ok();
            }

            // Iroh 消息接收
            msg = transport.recv(), if true => {
                match msg {
                    Some(wm) => {
                        let name = wm.sender_name;
                        let text = wm.text;
                        println!("\r[{}] 👤 {}: {}", now_ms() % 100000, name, text);
                        print!("> "); io::stdout().flush().ok();
                    }
                    None => {
                        // channel closed
                    }
                }
            }
        }
    }

    println!("\n👋 再见！");
    Ok(())
}
