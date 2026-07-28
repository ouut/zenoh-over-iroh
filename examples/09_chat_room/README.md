# zenoh-chat-room 部署指南

> 基于 zenoh × Iroh 的 P2P 控制台聊天室 — 从单机模拟到多机部署

---

## 快速体验（单机模拟）

无需任何外部依赖，直接在本地运行：

```bash
cd examples/09_chat_room
cargo run -- Alice lobby
```

输入 `/demo` 查看状态机在断网恢复场景下的行为。

---

## 生产部署架构

```
┌──────────────────────────────────────────────────────────────┐
│                        互联网                                 │
│                                                              │
│   ┌─────────┐    ┌─────────┐    ┌─────────┐                 │
│   │ Alice   │    │  Bob    │    │ Charlie │   ...            │
│   │ (移动4G)│    │ (家宽)  │    │ (咖啡WiFi)│                │
│   └────┬────┘    └────┬────┘    └────┬────┘                 │
│        │              │              │                       │
│        └──────────────┼──────────────┘                       │
│                       │                                      │
│                 ┌─────┴─────┐                                │
│                 │ iroh-relay │  (自建或官方 Relay)            │
│                 │ 保底中继    │                                │
│                 └───────────┘                                │
│                                                              │
│  优先: 节点间直连 (UDP Hole Punching)                         │
│  保底: Relay 转发 (端到端加密，Relay 无法解密)                 │
└──────────────────────────────────────────────────────────────┘
```

## 第 1 步: 部署 Relay（保底中继）

### 方案 A: 使用官方 Relay（零成本，开发用）

```bash
# 无需额外部署，应用自动连接 iroh.network 官方 Relay
# 适用于开发测试，不保证容量（风险 5.4）
```

### 方案 B: 自建 Relay（推荐生产用）

```bash
# 1. 下载 iroh-relay
curl -sL "https://github.com/n0-computer/iroh/releases/download/v1.0.3/iroh-relay-v1.0.3-x86_64-unknown-linux-gnu.tar.gz" \
  | tar xz -C /usr/local/bin/

# 2. 生成密钥
SECRET=$(openssl rand -hex 32)

# 3. 启动（开发模式）
iroh-relay --dev

# 4. 生产部署（Docker）
docker run -d --name iroh-relay \
  -p 8080:8080 -p 8443:8443 \
  -e IROH_RELAY_HTTP_ADDR=0.0.0.0:8080 \
  -e IROH_RELAY_SECRET_KEY=$SECRET \
  ghcr.io/n0-computer/iroh-relay:v1.0.3
```

详见 `doc/自建Relay部署方案.md`。

## 第 2 步: 编译聊天室应用

```bash
# 1. 确保 zenoh-link-iroh 插件已编译
cd /path/to/zenoh-link-state
cargo build --release

# 2. 编译聊天室
cd examples/09_chat_room
cargo build --release
```

## 第 3 步: 配置 zenohd + Iroh transport

```bash
# 1. 启动 Iroh Relay（如使用自建）
iroh-relay --dev &

# 2. 启动 zenohd 并加载 iroh_link 插件
zenohd \
  -P iroh_link \
  -l iroh/<your_node_id> \
  -c zenoh-chat.json5
```

`zenoh-chat.json5` 配置:

```json5
{
  mode: "peer",
  listen: {
    endpoints: ["iroh/<你的NodeID>"]
  },
  transport: {
    link: {
      iroh: {
        relay_url: "http://localhost:3340"  // 或自建 Relay URL
      }
    }
  }
}
```

## 第 4 步: 运行聊天室

```bash
# 用户 Alice (移动热点)
cargo run --release -- Alice lobby

# 用户 Bob (家用宽带)
cargo run --release -- Bob lobby

# 用户 Charlie (咖啡店 Wi-Fi)
cargo run --release -- Charlie lobby
```

## 第 5 步: 验证多设备互联

```bash
# 在任意设备上
> /users

📋 在线用户 (3 人):
   Alice [node_a1b2c3d4...]
   Bob   [node_e5f6a7b8...]
   Charlie [node_9c0d1e2f...] (我)
```

---

## NAT / 防火墙说明

| 场景 | 是否可达 | 原理 |
|------|:---:|------|
| 双方均为普通 NAT | ✅ 大概率直连 | UDP Hole Punching |
| 一方对称 NAT + 一方锥形 NAT | ✅ 可直连 | 锥形 NAT 端口可预测 |
| 双方对称 NAT | ⚠️ 需 Relay | 对称 NAT 端口不可预测 |
| 企业防火墙（仅允许 80/443） | ✅ Relay | QUIC over HTTPS (443) |

> **Relay 安全保证**: 即使通过 Relay 转发，所有消息端到端加密（QUIC TLS 1.3），Relay 全程无法解密。

---

## 多房间支持

```bash
# 不同房间名 = 不同 zenoh key prefix
cargo run -- Alice dev-team     # 开发团队频道
cargo run -- Alice ops-team     # 运维团队频道
cargo run -- Bob   dev-team     # Bob 加入开发团队
```

房间间完全隔离，zenoh key: `chat/<room>/messages`。

---

## 状态机行为演示

在聊天室中输入 `/demo` 观察：

```
> /demo

🎬 演示: 网络切换场景
────────────────────────
[1/4] 正在迁移 (网络切换中)...
[2/4] 断网期间排队: 这条消息在断网期间排队，恢复后发送
[3/4] 网络恢复，排队数据已发送 ✓
[4/4] 连接状态: Connected
```

设计要点：
- **断网期间**：消息被 `LinkStateMachine` 排队（不报错，用户无感知）
- **网络恢复**：排队消息自动发送，无丢失
- **超时保护**：若 4s 未恢复 → 进入 Disconnected，提示用户"连接已断开"

---

## 运维

### 监控连接状态

```bash
# zenohd 日志（含路径迁移事件）
zenohd -P iroh_link 2>&1 | grep "path_migrated\|path_restored\|MigrationTimeout"
```

### 查看在线用户

```
> /users
```

### 消息历史

最近 200 条消息保存在内存中（`ChatRoom.history`）。

---

## 扩展方向

- **文件传输**: `zenoh` 支持大 payload (默认 1GB)，可直接分享文件
- **端到端加密**: 应用层二次加密（Iroh QUIC 已提供传输层加密）
- **持久化**: 接入 `zenoh-plugin-storage` 做消息持久化
- **Web 前端**: 替换 stdin/stdout 为 WebSocket + 浏览器 UI
- **语音/视频**: Iroh 支持 QUIC datagram，可扩展 WebRTC-like 实时流
