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

---

## 不同 Zenoh 拓扑下的行为

`LinkStateMachine` 作用于**单条 Link**（单条 Iroh QUIC 连接）。拓扑结构决定有多少条 Link 同时存在，每条 Link 有独立的状态机。

### 1. Peer-to-Peer (对等模式)

```
  Node A ←──iroh──→ Node B
  ┌─────┐          ┌─────┐
  │ LSM │          │ LSM │    每条连接一个状态机
  └─────┘          └─────┘
```

```rust
// A 和 B 都这样配置
zenoh::open(Config {
    mode: Mode::Peer,
    connect: Endpoints::from("iroh/<B的NodeID>"),
    listen: Endpoints::from("iroh/<A的NodeID>"),
}).await?;

// 状态机自动管理 A↔B 之间的连接迁移
```

| 场景 | 状态机行为 |
|------|----------|
| A 切 Wi-Fi→4G | A 侧的 LSM 进入 Migrating，消息排队 |
| B 正常 | B 侧的 LSM 保持 Connected，无影响 |
| A 恢复 | LSM 回到 Connected，排队消息自动发送 |

### 2. Client-Router (路由模式)

```
  Client A ──iroh──→ Router ──iroh──→ Client B
  ┌─────┐         ┌────────┐         ┌─────┐
  │ LSM │         │ LSM LSM│         │ LSM │
  └─────┘         └────────┘         └─────┘
```

```rust
// Router (服务器)
zenohd -P iroh_link -l iroh/<router_node_id>

// Client A
zenoh::open(Config {
    mode: Mode::Client,
    connect: Endpoints::from("iroh/<router_node_id>"),
}).await?;

// Client B 同理
```

三条 Link、三个独立状态机：

| Link | 若 A 侧切换网络 | 影响 |
|------|:---:|------|
| A↔Router | A 侧 LSM → Migrating | 仅 A↔Router 受影响 |
| Router↔B | Router 侧 LSM 不变 | Router↔B 不受影响 |
| A↔B 的消息 | Router 自动转发 | B 无感知 |

### 3. Mesh (网状模式)

```
     A ──── B
     │ \   / │
     │  \ /  │
     │   X   │     每个节点 2-3 条 Link
     │  / \  │     每条 Link 独立状态机
     │ /   \ │
     C ──── D
```

```rust
// 每个节点配置多个 connect
zenoh::open(Config {
    mode: Mode::Peer,
    connect: Endpoints::from(vec![
        "iroh/<B>", "iroh/<C>", "iroh/<D>",
    ]),
}).await?;
```

| 场景 | 行为 |
|------|------|
| C 断网 | A↔C LSM → Migrating，消息走 A→B→C 路径 |
| C 恢复 | A↔C LSM → Connected，排队数据自动排出 |
| C 永久断开 | LSM 超时 → Disconnected，Zenoh 路由剔除节点 |

> **关键**：状态机只管单条 Link。多路径容灾是 Zenoh 路由层的事。

### 4. 移动端（单 Client 切网）

```
  手机 A ──iroh──→ Relay/Peer
  ┌─────────┐
  │ LSM × 1 │   移动端通常只有一条 Link
  └─────────┘
```

最典型场景——状态机价值最大：

```
Wi-Fi → 4G 切换:
  LSM.on_path_change(false)  // Migrating
  应用继续 put()              // 消息排队
  LSM.on_path_change(true)   // 恢复, drain()
  
4G → Wi-Fi 切换:
  同上，用户无感知
```

### 总结

| 拓扑 | Link 数量 | LSM 实例数 | 故障隔离 |
|------|:---:|:---:|:---:|
| Peer-to-Peer | 1 | 2 (每端1个) | 单连接 |
| Client-Router | 2 | 3 | 逐 Link 隔离 |
| Mesh (3节点) | 3 | 6 | 故障 Link 自动绕路 |
| 移动端 | 1 | 1 | 单 Link 迁移 |

---

## bindings 的定位：给谁用的？

### ❌ 不是给最终用户的

如果你用 Zenoh pub/sub，绑定层跟你无关：

```rust
// 你只需要这样——和用 TCP 没区别
let session = zenoh::open(config).await?;
session.put("demo", "hello").await?;
```

### ✅ 是给传输层实现者的

当你需要**自己实现一个新的 zenoh transport**（比如用 QUIC、蓝牙、LoRa），你需要 LinkStateMachine 来处理连接迁移。这时候 bindings 让你可以从 Python/Swift/Kotlin 调用它。

```
┌─────────────────────────────────────┐
│  你的自定义 Transport (Python 写的)   │
│  ├─ 你的连接管理代码                   │
│  ├─ zenoh_link_state.LinkStateMachine│  ← 这里用 bindings
│  └─ 你的网络 IO                      │
└─────────────────────────────────────┘
```

---

## 移动端方案：编译一个包含一切的 lib

你的理解是对的——**编译一个包含 zenoh + iroh + state machine 的 lib，然后从 Swift/Kotlin 调 Zenoh C API**。

```
源代码:
  zenoh crate (pub/sub 协议)
  + zenoh-link-iroh (插件: state machine + iroh)
  + iroh crate (QUIC P2P)
  ─────────────────────────────────────
  编译为一个静态库: libzenoh_full.a

iOS (Swift):
  libzenoh_full.a → Bridging Header → Swift
  API: zenoh_open(), zenoh_put(), zenoh_subscribe()

Android (Kotlin):
  libzenoh_full.so → JNI → Kotlin
  API: Zenoh.open(), session.put(), session.subscribe()
```

### 具体步骤

```bash
# 1. 创建移动端 workspace Cargo.toml
[lib]
name = "zenoh_mobile"
crate-type = ["staticlib", "cdylib"]

[dependencies]
zenoh = "1"
iroh = "0.32"

# 2. 写 FFI 绑定层 (暴露 Zenoh API, 不是 LinkStateMachine)
#[no_mangle]
pub extern "C" fn zenoh_put(key: *const c_char, value: *const c_char) { ... }

# 3. 编译
cargo build --target aarch64-apple-ios --release
# → libzenoh_mobile.a  (包含 zenoh + iroh + state machine)

# 4. 集成到 Xcode / Android Studio
# Swift 调用: zenoh_put("demo/test", "hello")
```

**用户只需学会 Zenoh 的 pub/sub 概念，不需要知道 iroh 或 state machine 的存在。**
