# Mobile 端集成方案

> zenoh-link-iroh 如何被手机使用：iOS 和 Android。

---

## 核心问题

桌面用 `zenohd -P iroh_link` 加载 `.so`，但手机**没有 zenohd**。手机是把 `zenoh` + `iroh` 作为静态库链接到 App 里的。

## iOS 方案

### Step 1: 创建一个 Rust 移动端 Lib

```bash
# 新项目目录（不在本 repo 里，是另一个 Cargo workspace）
mkdir zenoh-mobile && cd zenoh-mobile

# Cargo.toml
cat > Cargo.toml << 'EOF'
[package]
name = "zenoh_mobile"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["staticlib"]       # → .a 静态库

[dependencies]
zenoh = "1"
iroh = "0.32"
zenoh-link-state = { git = "https://github.com/ouut/zenoh-over-iroh" }
EOF
```

### Step 2: 写 FFI 层（暴露 Zenoh C API）

```rust
// src/lib.rs — 把 Zenoh 的 session/zid/put/subscribe 暴露为 C 函数
use std::ffi::{CStr, CString};
use std::sync::OnceLock;
use zenoh::prelude::r#async::*;

static SESSION: OnceLock<zenoh::Session> = OnceLock::new();

#[no_mangle]
pub extern "C" fn zenoh_mobile_open(yaml_config: *const std::os::raw::c_char) -> i32 {
    let c_str = unsafe { CStr::from_ptr(yaml_config) };
    let config_str = c_str.to_str().unwrap();

    let rt = tokio::runtime::Runtime::new().unwrap();
    let session = rt.block_on(async {
        // "listen": ["iroh/0.0.0.0:0"]  ← 这一行启用 iroh 传输层
        zenoh::open(zenoh::Config::from_str(config_str).unwrap()).await.unwrap()
    });

    SESSION.set(session).map(|_| 0).unwrap_or(-1)
}

#[no_mangle]
pub extern "C" fn zenoh_mobile_put(key: *const std::os::raw::c_char, value: *const std::os::raw::c_char) -> i32 {
    let session = match SESSION.get() { Some(s) => s, None => return -1 };
    let k = unsafe { CStr::from_ptr(key).to_str().unwrap() };
    let v = unsafe { CStr::from_ptr(value).to_str().unwrap() };
    rt().block_on(session.put(k, v)).map(|_| 0).unwrap_or(-1)
}
```

### Step 3: 编译

```bash
# iOS 真机
cargo build --release --target aarch64-apple-ios
# 产出: target/aarch64-apple-ios/release/libzenoh_mobile.a

# iOS 模拟器
cargo build --release --target aarch64-apple-ios-sim

# 合并
lipo -create -output libzenoh_mobile.a \
  target/aarch64-apple-ios/release/libzenoh_mobile.a \
  target/aarch64-apple-ios-sim/release/libzenoh_mobile.a
```

### Step 4: Xcode 集成

```swift
// Swift — 和桌面版一样的 Zenoh API
let config = """
{
  mode: "peer",
  listen: { endpoints: ["iroh/0.0.0.0:0"] }
}
"""

// 初始化（传入 config，iroh 传输层在内）
zenoh_mobile_open(config)

// — 以下就是标准 Zenoh API —
zenoh_mobile_put("sensor/temp", "25.5")
```

**整个 Iroh 传输层编译在这个 `.a` 里，包括 LinkStateMachine。**

## Android 方案

### 一样的 Rust 代码，编译为 .so

```bash
cargo build --release --target aarch64-linux-android
# 产出: target/aarch64-linux-android/release/libzenoh_mobile.so
```

### Kotlin 调用

```kotlin
// JNI 层
class ZenohSession {
    external fun open(config: String): Int
    external fun put(key: String, value: String): Int
    external fun subscribe(key: String, callback: (String, String) -> Unit): Int
    external fun close()
}

// 使用
val session = ZenohSession()
session.open("""
    mode: "peer"
    listen: { endpoints: ["iroh/0.0.0.0:0"] }
""")
session.put("chat/lobby", "hello from Android")
```

## 跟桌面端的关系

```
桌面端 (zenohd)          手机端 (App)
─────────────────        ─────────────────
zenohd -P iroh_link       libzenoh_mobile.a (静态链接)
      │                          │
      ▼                          ▼
zenoh 传输系统             zenoh 传输系统
      │                          │
      ▼                          ▼
iroh QUIC P2P             iroh QUIC P2P
```

两边 Iroh 互通，不需要额外适配。

## 关键原则

| 问题 | 答案 |
|------|------|
| 手机能不能用 `"iroh/"`？ | 可以，编译为静态库后链路层就在里面 |
| 用户在手机上写什么 API？ | `session.put("k","v")` — 和桌面完全一样 |
| iroh 在手机端需要额外配置吗？ | 不需要，编译进去后自动生效 |
| 需不需要单独编译 iroh 的 SDK？ | 不需要，全部在一个 `.so` / `.a` 里 |
| Swift/Kotlin 怎么写？ | 通过 C FFI 桥接层调 Zenoh API |
