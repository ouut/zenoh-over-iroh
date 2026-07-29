# 使用 Zenoh over Iroh 的正确方式

> 用户只需 Zenoh API。Iroh 是透明的传输层，不需要单独学习。

---

## 一张图说清楚

```
你的代码
   │
   │  session.put("sensor/temp", b"25°C")
   │  session.declare_subscriber("room/*")
   ▼
┌──────────────────────────────┐
│  Zenoh API (你学的全部)        │
│  pub / sub / query / key-expr │
└──────────┬───────────────────┘
           │
┌──────────▼───────────────────┐
│  zenoh-link-iroh (插件)       │  ← 自动加载，你不需要碰
│  ├─ LinkStateMachine          │
│  └─ iroh::Endpoint            │
└──────────┬───────────────────┘
           │
┌──────────▼───────────────────┐
│  Iroh QUIC P2P               │  ← 透明传输
│  (打洞 + Relay 保底)          │
└──────────────────────────────┘
```

---

## Rust 桌面应用

### Cargo.toml

```toml
[dependencies]
zenoh = "1"                           # Zenoh pub/sub API
# zenoh-link-iroh 插件会在运行时自动加载
```

### 代码

```rust
use zenoh::prelude::r#async::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 配置: 使用 iroh transport
    let config = r#"
    {
      mode: "peer",
      listen: { endpoints: ["iroh/0.0.0.0:0"] },
      connect: { endpoints: [] }
    }
    "#;

    let session = zenoh::open(zenoh::Config::from_str(config)?).await?;

    // ── 以下就是标准 Zenoh API，和用 TCP 没任何区别 ──

    // 发布
    session.put("chat/lobby", "hello").await?;

    // 订阅
    let sub = session.declare_subscriber("chat/lobby").await?;
    while let Ok(msg) = sub.recv_async().await {
        println!("收到: {}", msg.payload().to_string());
    }

    Ok(())
}
```

**关键**: `listen: { endpoints: ["iroh/..."] }` — 只需要这一行配置，传输层就切到 Iroh。其他代码和 TCP 完全一样。

---

## Python 桌面应用

### 安装

```bash
pip install eclipse-zenoh
```

### 代码

```python
import zenoh

# 配置: iroh transport
config = {
    "mode": "peer",
    "listen": {"endpoints": ["iroh/0.0.0.0:0"]},
}

session = zenoh.open(config)

# 发布
session.put("chat/lobby", "hello from Python")

# 订阅
sub = session.declare_subscriber("chat/lobby")
for msg in sub:
    print(f"收到: {msg.payload.decode()}")
```

---

## iOS (Swift)

### 编译一个 lib

```bash
# 1. 创建包含 zenoh + iroh 的 Rust workspace
# 2. 编译为 iOS staticlib
cargo build --release --target aarch64-apple-ios

# 产出: libzenoh_mobile.a (包含 zenoh pub/sub + state machine + iroh QUIC)
```

### Swift 调用（通过 zenoh C API）

```swift
// 配置 iroh transport
let config = """
{
  mode: "peer",
  listen: { endpoints: ["iroh/0.0.0.0:0"] },
}
"""

let session = zenoh_open(config)

// 发布消息 — 标准 Zenoh API
zenoh_put(session, "sensor/temp", "25.5°C")

// 订阅消息
zenoh_subscribe(session, "sensor/*") { key, value in
    print("收到: \(key) = \(value)")
}
```

---

## Android (Kotlin)

### 编译

```bash
cargo build --release --target aarch64-linux-android
# 产出: libzenoh_mobile.so
```

### Kotlin (JNI)

```kotlin
// 配置
val config = """
{
  mode: "peer",
  listen: { endpoints: ["iroh/0.0.0.0:0"] },
}
"""

val session = Zenoh.open(config)

// 发布
session.put("chat/lobby", "hello from Android")

// 订阅
session.subscribe("chat/lobby") { msg ->
    println("收到: ${msg.payload}")
}
```

---

## 配置速查

| 配置项 | 值 | 说明 |
|------|-----|------|
| `listen.endpoints` | `["iroh/0.0.0.0:0"]` | 监听 Iroh 连接，随机端口 |
| `connect.endpoints` | `["iroh/<对端NodeID>"]` | 连接到指定节点 |
| `mode` | `"peer"` / `"client"` | 对等/客户端模式 |

不配 `iroh/` 前缀则默认用 TCP——和原生 Zenoh 完全兼容。

---

## 总结

| 你做 | 你不用管 |
|------|---------|
| `session.put("k","v")` | QUIC 握手 |
| `session.declare_subscriber("k")` | UDP 打洞 |
| 配置里写 `"iroh/..."` | Relay 切换 |
| — | LinkStateMachine |
| — | 路径迁移 |
