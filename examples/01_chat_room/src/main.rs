//! 01_chat_room — P2P Chat using iroh + LinkStateMachine
//!
//! Run two terminals:
//!   Terminal A:  cargo run -- Alice
//!   Terminal B:  cargo run -- Bob
//!   > /connect <other's NodeID>
//!   Then chat!

mod chat;
use chat::ChatMessage;
use std::io::{self, BufRead, Write};
use tracing::{info, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_writer(io::stderr).init();

    let name = std::env::args().nth(1).unwrap_or_else(|| "Anonymous".into());

    let ep = iroh::Endpoint::builder()
        .discovery_n0()
        .relay_mode(iroh::RelayMode::Default)
        .alpns(vec![b"chat/1.0".to_vec()])
        .bind().await?;

    let node_id = ep.node_id().to_string();
    info!(%node_id, "Iroh endpoint ready");

    println!("\n╔══════════════════════════════════════╗");
    println!("║  P2P Chat — Iroh QUIC              ║");
    println!("╠══════════════════════════════════════╣");
    println!("║ {}  @  {}  ║", name, &node_id[..28]);
    println!("╚══════════════════════════════════════╝");
    println!("  /connect <NodeID>");
    println!("  /quit\n");

    // Accept loop
    let ep2 = ep.clone();
    tokio::spawn(async move {
        while let Some(incoming) = ep2.accept().await {
            tokio::spawn(async move {
                let conn = match incoming.await {
                    Ok(c) => c, Err(_) => return
                };
                let remote = conn.remote_node_id().unwrap().to_string();
                loop {
                    match conn.accept_bi().await {
                        Ok((_send, mut recv)) => {
                            let mut buf = vec![0u8; 65536];
                            if let Ok(buf) = recv.read_to_end(65536).await {
                                let msg = String::from_utf8_lossy(&buf).to_string();
                                if !msg.is_empty() {
                                    println!("\r📩 {}: {}", &remote[..12], msg);
                                    print!("> "); io::stdout().flush().ok();
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
        }
    });

    // Read stdin + send
    let peer = std::sync::Arc::new(tokio::sync::Mutex::new(None::<String>));
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    std::thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            if let Ok(s) = line { let _ = tx.send(s); }
        }
    });

    while let Some(input) = rx.recv().await {
        let input = input.trim().to_string();
        if input.is_empty() { continue; }

        if input == "/quit" { break; }

        if let Some(rest) = input.strip_prefix("/connect ") {
            match rest.trim().parse::<iroh::NodeId>() {
                Ok(id) => {
                    *peer.lock().await = Some(rest.to_string());
                    match ep.connect(id, b"chat/1.0").await {
                        Ok(_) => println!("✅ Connected to {}", rest),
                        Err(e) => println!("❌ {}", e),
                    }
                }
                Err(_) => println!("❌ Invalid NodeID"),
            }
            continue;
        }

        let target = peer.lock().await.clone();
        match target {
            Some(ref id) => {
                match id.parse::<iroh::NodeId>() {
                    Ok(nid) => {
                        if let Ok(conn) = ep.connect(nid, b"chat/1.0").await {
                            if let Ok((mut send, _)) = conn.open_bi().await {
                                send.write_all(input.as_bytes()).await.ok();
                                send.finish()?;
                            }
                        }
                    }
                    Err(_) => {}
                }
            }
            None => println!("  /connect <NodeID> first"),
        }
    }

    ep.close().await;
    Ok(())
}
