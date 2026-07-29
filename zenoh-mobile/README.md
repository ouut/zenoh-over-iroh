# Mobile: Zenoh × Iroh 移动端集成

> 一个 `.a` / `.so` 包含 Zenoh 核心 + Iroh 传输层 + LinkStateMachine。

---

## 架构

```
你的 Swift / Kotlin 代码
       │ session.put("k","v")
       ▼
┌──────────────────────────────┐
│  libzenoh_mobile.a / .so      │  ← 编译产物
│                              │
│  ├─ zenoh pub/sub             │
│  ├─ iroh QUIC P2P 传输层      │  ← 全部静态链接
│  └─ LinkStateMachine          │
└──────────────────────────────┘
```

**不对接官方 `zenoh-c`**——因为 `zenoh-c` 只包含官方 Zenoh，没有 Iroh 传输层。
我们自己编译一个包含一切的 lib，API 和官方完全一致。

---

## 编译

```bash
cd mobile

# iOS 真机
cargo build --release --target aarch64-apple-ios

# Android arm64
cargo build --release --target aarch64-linux-android
```

## 产物

| 平台 | 文件 | 大小 |
|------|------|------|
| iOS | `../target/aarch64-apple-ios/release/libzenoh_mobile.a` | ~20MB (release) |
| Android | `../target/aarch64-linux-android/release/libzenoh_mobile.so` | ~10MB (release) |

## 集成

### iOS (Xcode)

1. 将 `libzenoh_mobile.a` + `ios/ZenohMobile.h` 拖入 Xcode
2. Swift 中调用:

```swift
// 配置：iroh 传输层
ZenohMobile.open("""
{
  mode: "peer",
  listen: { endpoints: ["iroh/0.0.0.0:0"] }
}
""")

// 标准 Zenoh API
ZenohMobile.put(key: "sensor/temp", value: "25.5°C")
ZenohMobile.close()
```

### Android (Android Studio)

1. 将 `libzenoh_mobile.so` 放入 `app/src/main/jniLibs/arm64-v8a/`
2. Kotlin 中调用:

```kotlin
ZenohMobile.open("""
{
  mode: "peer",
  listen: { endpoints: ["iroh/0.0.0.0:0"] }
}
""")
ZenohMobile.put("sensor/temp", "25.5°C")
ZenohMobile.close()
```

## 标准 Zenoh 配置

| 传输层 | 配置 | 适用场景 |
|--------|------|---------|
| TCP | `"listen": { "endpoints": ["tcp/0.0.0.0:0"] }` | 内网开发 |
| Iroh P2P | `"listen": { "endpoints": ["iroh/0.0.0.0:0"] }` | 跨 NAT 生产 |

## 文件结构

```
mobile/
├── Cargo.toml           # 包含 zenoh + iroh + link-state
├── src/lib.rs           # FFI: open / put / close
├── ios/
│   ├── ZenohMobile.h    # ObjC Bridging Header
│   ├── ZenohMobile.m    # ObjC 包装
│   └── HelloWorld.swift # 示例
├── android/
│   └── ZenohMobile.kt   # JNI 包装
└── README.md            # 本文档
```

---

## Python 支持

`.so` 编译后，通过 Python ctypes 直接调用，**无需 pip 安装任何包**：

```python
from mobile.python.zenoh import open_session, put, subscribe, close

s = open_session('{"listen": {"endpoints": ["tcp/127.0.0.1:0"]}}')
put(s, "hello", "world")

def on_msg(key, value):
    print(f"收到: {key} = {value}")

subscribe(s, "demo/test", on_msg)
close(s)
```

## 多语言支持

产出物 `libzenoh_mobile.so` / `.a` 是标准 C ABI，任何支持 FFI 的语言均可使用：

| 语言 | 方案 | 代码量 |
|------|------|:---:|
| C | 直接包含 `include/zenoh_mobile.h` | 0 |
| Python | `python/zenoh.py` (ctypes) | 50 行 |
| Swift | `swift/ZenohMobile.h` / `.m` | 20 行 |
| Kotlin | `kotlin/ZenohMobile.kt` (JNI) | 15 行 |
| Lua | `ffi.load("libzenoh_mobile")` | 10 行 |
| Node.js | `ffi-napi` 加载 `.so` | 20 行 |
