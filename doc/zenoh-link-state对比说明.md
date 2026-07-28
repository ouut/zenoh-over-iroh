# zenoh-link-state vs Zenoh vs Iroh — 关系与区别

## 一句话总结

> **zenoh**: 应用层消息协议（Pub/Sub/Query）  
> **iroh**: P2P 传输层（QUIC + 打洞 + Relay）  
> **zenoh-link-state**: 让 zenoh 和 iroh 正确协同的状态机（解决 QUIC 迁移 vs Zenoh 断连的语义冲突）  

---

## 对比表

| 维度 | Zenoh | Iroh | zenoh-link-state |
|------|-------|------|:---:|
| **是什么** | 消息中间件 | P2P 网络库 | 状态机（360 行 Rust） |
| **层次** | 应用层 (L7) | 传输层 (L4) | 胶水层 |
| **解决的问题** | "数据怎么路由到订阅者" | "两个节点怎么建立连接" | "连接迁移时 Zenoh 不要误断连" |
| **API 风格** | `session.declare_publisher(key).put(data)` | `endpoint.connect(node_id, alpn)` | `lsm.on_path_change(false)` |
| **依赖数量** | ~200 crates | ~180 crates | **2 crates** (tokio + tracing) |
| **编译时间** | ~5 min | ~3 min | **~2 秒** |
| **独立使用** | ✅ 可以 | ✅ 可以 | ❌ 必须配合 zenoh + iroh |

---

## 在架构中的位置

```
┌─────────────────────────────────────────┐
│  Zenoh 应用代码                          │
│  session.put("sensor/temp", payload)    │  ← 用户直接调用 Zenoh API
└──────────────────┬──────────────────────┘
                   │
┌──────────────────▼──────────────────────┐
│  Zenoh 核心 (Router / Session)          │
│  - Pub/Sub 路由                          │
│  - Key-Expression 匹配                   │  ← Zenoh 负责消息路由
│  - QoS、压缩、分片                       │
└──────────────────┬──────────────────────┘
                   │  调用 LinkUnicast.write()
                   ▼
┌─────────────────────────────────────────┐
│  zenoh-link-state ← 本项目               │
│  ┌─────────────────────────────────┐    │
│  │ LinkStateMachine                │    │  ← 状态过滤：路径迁移 → 不报错
│  │ Connected/Migrating/Disconnected│    │
│  └─────────────────────────────────┘    │
└──────────────────┬──────────────────────┘
                   │
┌──────────────────▼──────────────────────┐
│  Iroh                                    │
│  - QUIC 连接 (TLS 1.3)                   │  ← Iroh 负责传输
│  - UDP 打洞                             │
│  - Relay 保底转发                        │
└─────────────────────────────────────────┘
```

---

## 为什么需要这个中间层？

### 问题

Zenoh 的 Link 模型只有两个状态：**活着** / **断了**。  
Iroh 的 QUIC 连接有**路径迁移**：IP 变了但连接没断。

```
Zenoh 以为:  [Connected] ──断连──→ [Dead] → 触发重连
Iroh 实际:  [Connected] ──迁移──→ [Connected] (IP变了，连接还在)
```

**没有状态机**: Zenoh 看到 Iroh 路径切换，误判断连，触发不必要的重连。

**有状态机**: Migrating 期间不上报 Error，超时才断开，避免误判。

### 效果

| 场景 | 无状态机 | 有状态机 |
|------|:---:|:---:|
| 移动设备 Wi-Fi → 4G (2s) | Zenoh 断开 + 重连 | 消息排队，恢复后自动发送 |
| 短暂网络抖动 (0.5s) | Zenoh 断开 + 重连 | 排队，0.5s 后恢复 |
| 真正断开 (>4s) | 正确断开 | 正确断开（排队数据作废） |

---

## 使用对比

### Zenoh 用法

```rust
// 打开 session
let session = zenoh::open(config).await.unwrap();

// 发布消息
let publisher = session.declare_publisher("room/chat").await.unwrap();
publisher.put("hello").await.unwrap();

// 订阅消息
let subscriber = session.declare_subscriber("room/chat").await.unwrap();
while let Ok(sample) = subscriber.recv_async().await {
    println!("收到: {}", sample.payload());
}
```

### zenoh-link-state 用法

```rust
use zenoh_link_state::link_state::LinkStateMachine;

let mut lsm = LinkStateMachine::new();

// Iroh 路径切换回调 → 通知状态机
lsm.on_path_change(false);  // 失联 → Migrating

// Zenoh 写入数据 → 状态机决定：排队 or 立即发送 or 拒绝
lsm.write(b"hello".to_vec());  // → Queued (排队)

// Iroh 路径恢复
lsm.on_path_change(true);  // → Connected

// 排出排队数据，交给 Iroh 重新发送
let drained = lsm.drain_queue();
```

---

## 依赖关系

```
你的应用
  ├── zenoh (pub/sub/query)
  │     ├── zenoh-link-state ← 本项目 (自动集成在 zenoh-link-iroh 插件中)
  │     └── zenoh-link-iroh (插件, 依赖 iroh)
  │           └── iroh (QUIC P2P)
  └── 其他 crates...
```

**最终用户不需要直接调用 `zenoh-link-state`**，它被嵌入在 `zenoh-link-iroh` 插件内部自动运行。

```rust
// 用户代码只需要这样
cargo add zenoh-link-state   // 或等待 zenoh-link-iroh 发布后自动包含
```
