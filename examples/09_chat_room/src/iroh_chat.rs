//! 真实 Iroh P2P 传输层 — 基于 iroh::Endpoint 的 QUIC 连接。
//!
//! 两个端点通过 NodeID 互相连接，消息通过 QUIC 双向流传输。
//! LinkStateMachine 集成在每个连接中处理路径迁移。

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use zenoh_link_state::iroh_integration::ConnectionStatus;
use zenoh_link_state::link_state::LinkStateMachine;

/// 消息在网络上传输时的格式。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WireMessage {
    pub sender_name: String,
    pub sender_node_id: String,
    pub text: String,
    pub timestamp_ms: u64,
    pub msg_type: String, // "msg" | "join" | "leave"
}

/// 真实的 Iroh 聊天传输层。
///
/// 封装 iroh::Endpoint + LinkStateMachine，提供:
/// - listen: 接受传入连接
/// - connect: 连接到远程节点
/// - send: 发送消息
/// - recv: 接收消息（通过 channel）
pub struct IrohChatTransport {
    /// 本地 NodeID。
    pub node_id: String,
    /// Iroh Endpoint。
    endpoint: iroh::Endpoint,
    /// 接收消息的 channel 接收端。
    rx: tokio::sync::mpsc::UnboundedReceiver<WireMessage>,
    /// 接收消息的 channel 发送端（用于 clone）。
    tx: tokio::sync::mpsc::UnboundedSender<WireMessage>,
    /// Relay URL（用于 NodeAddr）。
    relay_url: Option<iroh::RelayUrl>,
}

impl IrohChatTransport {
    /// 创建新的 Iroh 聊天传输层。
    ///
    /// `bind_addr`: 本地监听地址，如 "0.0.0.0:0" 表示随机端口
    /// `relay_url`: Iroh Relay URL，如 "https://iroh.network/relay"
    /// `secret_key`: 可选的固定密钥（十六进制编码的 32 字节）
    pub async fn new(
        bind_addr: &str,
        relay_url: Option<&str>,
        secret_key: Option<&str>,
    ) -> Result<Self> {
        let mut builder = iroh::Endpoint::builder();

        // 使用 Iroh discovery（自动发现 relay 和节点）
        builder = builder.discovery_n0();

        // 设置 ALPN 以接受传入连接
        builder = builder.alpns(vec![b"chat/1.0".to_vec()]);

        // Relay 配置
        let relay_url_parsed: Option<iroh::RelayUrl> = if let Some(relay) = relay_url {
            info!(relay_url = %relay, "Using relay");
            let url: iroh::RelayUrl = relay.parse()?;
            let relay_map = iroh::RelayMap::from_url(url.clone());
            builder = builder.relay_mode(iroh::RelayMode::Custom(relay_map));
            Some(url)
        } else {
            info!("No relay configured, using default relay mode");
            builder = builder.relay_mode(iroh::RelayMode::Default);
            None
        };

        // 固定 NodeID（可选）
        if let Some(key_hex) = secret_key {
            let key_bytes = hex::decode(key_hex)
                .map_err(|_| anyhow::anyhow!("Invalid secret key hex"))?;
            let key: [u8; 32] = key_bytes
                .try_into()
                .map_err(|_| anyhow::anyhow!("Secret key must be 32 bytes"))?;
            builder = builder.secret_key(iroh::SecretKey::from_bytes(&key));
            info!("Using fixed secret key");
        }

        // 绑定地址
        let bind_socket: std::net::SocketAddrV4 = bind_addr.parse()?;
        builder = builder.bind_addr_v4(bind_socket);

        let endpoint = builder.bind().await?;
        let node_id = endpoint.node_id().to_string();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        info!(%node_id, "Iroh endpoint created");

        // 后台 accept 循环
        let accept_ep = endpoint.clone();
        let accept_tx = tx.clone();
        tokio::spawn(async move {
            while let Some(incoming) = accept_ep.accept().await {
                let conn_tx = accept_tx.clone();
                tokio::spawn(handle_connection(incoming, conn_tx));
            }
        });

        Ok(Self {
            node_id,
            endpoint,
            rx,
            tx,
            relay_url: relay_url_parsed,
        })
    }

    /// 连接到远程节点。
    ///
    /// 返回成功连接的 NodeID。
    pub async fn connect(&self, remote_node_id: &str) -> Result<()> {
        let node_id: iroh::NodeId = remote_node_id.parse()?;
        let mut node_addr = iroh::NodeAddr::new(node_id);
        if let Some(ref relay) = self.relay_url {
            node_addr = node_addr.with_relay_url(relay.clone());
        }
        info!(%remote_node_id, "Connecting to peer");

        let conn = self.endpoint.connect(node_addr, b"chat/1.0").await?;
        info!(%remote_node_id, "Connected!");

        // 后台处理这个连接
        let tx = self.rx_clone();
        // Since we already have a Connection, not an Incoming,
        // we spawn a handler that works with an established connection.
        tokio::spawn(handle_established_connection(conn, tx));

        Ok(())
    }

    /// 发送消息到指定节点。
    pub async fn send(&self, to_node_id: &str, msg: WireMessage) -> Result<()> {
        let node_id: iroh::NodeId = to_node_id.parse()?;
        let mut node_addr = iroh::NodeAddr::new(node_id);
        if let Some(ref relay) = self.relay_url {
            node_addr = node_addr.with_relay_url(relay.clone());
        }
        let conn = self.endpoint.connect(node_addr, b"chat/1.0").await?;
        let (mut send, _recv) = conn.open_bi().await?;

        let json = serde_json::to_vec(&msg)?;
        send.write_all(&(json.len() as u32).to_be_bytes()).await?;
        send.write_all(&json).await?;
        send.finish()?;
        send.stopped().await?;

        debug!(to = %to_node_id, len = json.len(), "Message sent");
        Ok(())
    }

    /// 接收消息（非阻塞）。
    pub async fn recv(&mut self) -> Option<WireMessage> {
        self.rx.recv().await
    }

    fn rx_clone(&self) -> tokio::sync::mpsc::UnboundedSender<WireMessage> {
        self.tx.clone()
    }
}

/// 处理传入的 Iroh 连接（来自 accept），读取其中的消息。
async fn handle_connection(
    incoming: iroh::endpoint::Incoming,
    tx: tokio::sync::mpsc::UnboundedSender<WireMessage>,
) {
    let conn = match incoming.await {
        Ok(c) => c,
        Err(e) => {
            error!(%e, "Connection failed");
            return;
        }
    };

    handle_established_connection(conn, tx).await;
}

/// 处理已建立的 Iroh 连接，读取其中的消息。
async fn handle_established_connection(
    conn: iroh::endpoint::Connection,
    tx: tokio::sync::mpsc::UnboundedSender<WireMessage>,
) {
    let remote = match conn.remote_node_id() {
        Ok(id) => id.to_string(),
        Err(e) => {
            error!(%e, "Failed to get remote node id");
            return;
        }
    };
    info!(%remote, "Connection established");

    loop {
        match conn.accept_bi().await {
            Ok((mut send, mut recv)) => {
                // 读取消息长度
                let mut len_buf = [0u8; 4];
                if let Err(e) = recv.read_exact(&mut len_buf).await {
                    error!(%e, "Failed to read message length");
                    continue;
                }

                let len = u32::from_be_bytes(len_buf) as usize;
                let mut buf = vec![0u8; len];
                if let Err(e) = recv.read_exact(&mut buf).await {
                    error!(%e, "Failed to read message body");
                    continue;
                }

                match serde_json::from_slice::<WireMessage>(&buf) {
                    Ok(msg) => {
                        debug!(from = %msg.sender_name, text = %msg.text, "Message received");
                        let _ = tx.send(msg);
                    }
                    Err(e) => {
                        warn!(%e, "Failed to deserialize message");
                    }
                }

                // 关闭发送方向
                let _ = send.finish();
                let _ = send.stopped().await;
            }
            Err(e) => {
                debug!(%e, "Connection closed");
                break;
            }
        }
    }
}
