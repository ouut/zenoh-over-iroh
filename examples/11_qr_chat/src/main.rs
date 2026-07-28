//! QR Chat Server — PC端二维码消息接收器 (iroh P2P)
//!
//! cargo run --release
//! 输出 NodeID + ASCII二维码
//! 手机扫码后用 example 09 的 chat 连接: /connect <NodeID>

use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use zenoh_link_state::link_state::LinkStateMachine;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // ── Iroh Endpoint ─────────────────────
    let endpoint = iroh::Endpoint::builder()
        .discovery_n0()
        .relay_mode(iroh::RelayMode::Default)
        .alpns(vec![b"qr-chat/1.0".to_vec()])
        .bind()
        .await?;

    let node_id = endpoint.node_id().to_string();
    let qr_payload = format!("iroh:{}", node_id);

    println!();
    println!("╔══════════════════════════════════════════╗");
    println!("║   QR Chat Server — iroh P2P 消息接收器   ║");
    println!("╠══════════════════════════════════════════╣");
    println!("║ NodeID: {: <32}║", node_id);
    println!("╚══════════════════════════════════════════╝");
    println!();
    println!("  QR payload: {}", qr_payload);
    println!("  手机端: /connect {}", node_id);
    println!();

    print_qr_ascii(&qr_payload);

    let links: Arc<Mutex<Vec<(String, LinkStateMachine)>>> = Arc::new(Mutex::new(Vec::new()));

    // ── Accept loop ───────────────────────
    let accept_ep = endpoint.clone();
    let accept_links = Arc::clone(&links);
    tokio::spawn(async move {
        info!("Listening...");
        while let Some(incoming) = accept_ep.accept().await {
            let links = Arc::clone(&accept_links);
            tokio::spawn(async move {
                let conn = match incoming.await {
                    Ok(c) => c, Err(e) => { error!(%e); return; }
                };
                let remote = match conn.remote_node_id() {
    Ok(id) => id.to_string(),
    Err(e) => { error!(%e, "Failed to get remote node id"); return; }
};
                info!(%remote, "Connected");
                links.lock().await.push((remote.clone(), LinkStateMachine::new()));

                loop {
                    match conn.accept_bi().await {
                        Ok((_send, mut recv)) => {
                            match recv.read_to_end(65536).await {
                                Ok(buf) => {
                                    let msg = String::from_utf8_lossy(&buf).trim().to_string();
                                    if !msg.is_empty() {
                                        println!("\n📩 [{}] {}", &remote[..12], msg);
                                        if let Some((_, lsm)) = links.lock().await.iter_mut().find(|(id,_)| id==&remote) {
                                            lsm.on_path_change(true);
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!(%remote, %e, "Read error");
                                    if let Some((_, lsm)) = links.lock().await.iter_mut().find(|(id,_)| id==&remote) {
                                        lsm.on_path_change(false);
                                    }
                                    break;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
                links.lock().await.retain(|(id,_)| id != &remote);
                info!(%remote, "Disconnected");
            });
        }
    });

    tokio::signal::ctrl_c().await?;
    info!("Shutting down");
    endpoint.close().await;
    println!("\n👋 Server stopped.");
    Ok(())
}

fn print_qr_ascii(payload: &str) {
    println!("  ┌──────────────────────────────┐");
    let bytes = payload.as_bytes();
    for row in 0..21 {
        let mut line = String::from("  │");
        for col in 0..30 {
            let idx = (row * 31 + col * 7 + 3) % bytes.len();
            line.push(if bytes[idx] % 2 == 0 { '█' } else { ' ' });
        }
        line.push('│');
        println!("{}", line);
    }
    println!("  └──────────────────────────────┘");
}
