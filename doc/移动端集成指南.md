# zenoh-link-state 移动端集成指南

> 将 `LinkStateMachine` 集成到 iOS (Swift) 和 Android (Kotlin) 应用

---

## 为什么可以直接支持移动端

| 要素 | 状态 |
|------|:---:|
| `LinkStateMachine` | ✅ 纯 Rust，零平台代码 |
| 依赖 `tokio` | ✅ 支持 iOS/Android |
| 依赖 `tracing` | ✅ 支持 iOS/Android |
| Cargo target | ✅ `aarch64-apple-ios`、`aarch64-linux-android` 等已安装 |

## 产出物

编译后得到三种库：

| 平台 | 库类型 | 文件名 |
|------|--------|--------|
| iOS (真机) | staticlib | `libzenoh_link_state.a` |
| iOS (模拟器) | staticlib | `libzenoh_link_state.a` |
| Android (arm64) | cdylib | `libzenoh_link_state.so` |

## iOS 集成

### 1. 编译 iOS 静态库

```bash
# 安装 iOS targets
rustup target add aarch64-apple-ios x86_64-apple-ios

# 编译真机版本
cargo build --release --target aarch64-apple-ios

# 编译模拟器版本
cargo build --release --target x86_64-apple-ios
```

### 2. 创建 C 头文件

```c
// zenoh_lsm.h — 供 Swift 引用

typedef void zenoh_lsm_t;

zenoh_lsm_t* zenoh_lsm_new(void);
zenoh_lsm_t* zenoh_lsm_new_with_backpressure(uint32_t max_queue);
void zenoh_lsm_free(zenoh_lsm_t* lsm);

int zenoh_lsm_on_path_change(zenoh_lsm_t* lsm, int connected);
int zenoh_lsm_write(zenoh_lsm_t* lsm, const uint8_t* data, uint32_t len);
int zenoh_lsm_can_read(zenoh_lsm_t* lsm);
int zenoh_lsm_tick(zenoh_lsm_t* lsm);
int zenoh_lsm_drain(zenoh_lsm_t* lsm, uint8_t* buf, uint32_t buf_len);
uint32_t zenoh_lsm_queue_len(zenoh_lsm_t* lsm);
int zenoh_lsm_is_connected(zenoh_lsm_t* lsm);
int zenoh_lsm_is_migrating(zenoh_lsm_t* lsm);
void zenoh_lsm_disconnect(zenoh_lsm_t* lsm);
```

### 3. Swift 封装

```swift
// LinkStateMachine.swift

import Foundation

/// Wraps the C FFI LinkStateMachine for Swift.
class LinkStateMachine {
    private var ptr: OpaquePointer?

    enum WriteStatus: Int {
        case sent = 0, queued = 1, backpressure = 2, disconnected = -1
    }

    enum LinkEvent: Int {
        case none = 0, pathMigrated = 1, pathRestored = 2, migrationTimeout = 3
    }

    init() {
        ptr = zenoh_lsm_new()
    }

    init(maxQueueDepth: UInt32) {
        ptr = zenoh_lsm_new_with_backpressure(maxQueueDepth)
    }

    deinit {
        if let p = ptr { zenoh_lsm_free(p) }
    }

    func onPathChange(connected: Bool) -> LinkEvent {
        LinkEvent(rawValue: Int(zenoh_lsm_on_path_change(ptr, connected ? 1 : 0)))!
    }

    func write(data: Data) -> WriteStatus {
        data.withUnsafeBytes { buf in
            WriteStatus(rawValue: Int(zenoh_lsm_write(ptr, buf.baseAddress?.assumingMemoryBound(to: UInt8.self), UInt32(data.count))))!
        }
    }

    var isConnected: Bool { zenoh_lsm_is_connected(ptr) != 0 }
    var isMigrating: Bool { zenoh_lsm_is_migrating(ptr) != 0 }
    var queueLength: UInt32 { zenoh_lsm_queue_len(ptr) }

    func tick() -> LinkEvent {
        LinkEvent(rawValue: Int(zenoh_lsm_tick(ptr)))!
    }

    func disconnect() { zenoh_lsm_disconnect(ptr) }
}
```

### 4. Xcode 配置

1. 将 `libzenoh_link_state.a` 和 `zenoh_lsm.h` 拖入 Xcode
2. Build Settings → Library Search Paths → 添加 `.a` 所在目录
3. Build Settings → Bridging Header → 指向 `zenoh_lsm.h`
4. 确保 Deployment Target ≥ iOS 13

---

## Android 集成

### 1. 编译 Android .so

```bash
# 安装 Android targets + NDK
rustup target add aarch64-linux-android armv7-linux-androideabi
export ANDROID_NDK_HOME="$HOME/Android/Sdk/ndk/26.1.10909125"

# 创建 cargo config
mkdir -p .cargo
cat > .cargo/config.toml << EOF
[target.aarch64-linux-android]
linker = "$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android21-clang"

[target.armv7-linux-androideabi]
linker = "$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/armv7a-linux-androideabi21-clang"
EOF

# 编译
cargo build --release --target aarch64-linux-android
```

### 2. Kotlin JNI 封装

```kotlin
// LinkStateMachine.kt

class LinkStateMachine(maxQueueDepth: Int = 0) {
    private external fun nativeNew(): Long
    private external fun nativeNewWithBackpressure(maxQueue: Int): Long
    private external fun nativeFree(ptr: Long)
    private external fun nativeOnPathChange(ptr: Long, connected: Boolean): Int
    private external fun nativeWrite(ptr: Long, data: ByteArray): Int
    private external fun nativeTick(ptr: Long): Int
    private external fun nativeQueueLen(ptr: Long): Int
    private external fun nativeIsConnected(ptr: Long): Boolean
    private external fun nativeIsMigrating(ptr: Long): Boolean
    private external fun nativeDisconnect(ptr: Long)

    private var ptr: Long = 0

    enum class WriteStatus(val code: Int) {
        SENT(0), QUEUED(1), BACKPRESSURE(2), DISCONNECTED(-1)
    }

    enum class LinkEvent(val code: Int) {
        NONE(0), PATH_MIGRATED(1), PATH_RESTORED(2), MIGRATION_TIMEOUT(3)
    }

    init {
        System.loadLibrary("zenoh_link_state")
        ptr = if (maxQueueDepth > 0) nativeNewWithBackpressure(maxQueueDepth) else nativeNew()
    }

    fun onPathChange(connected: Boolean): LinkEvent =
        LinkEvent.values().find { it.code == nativeOnPathChange(ptr, connected) }!!

    fun write(data: ByteArray): WriteStatus =
        WriteStatus.values().find { it.code == nativeWrite(ptr, data) }!!

    fun tick(): LinkEvent =
        LinkEvent.values().find { it.code == nativeTick(ptr) }!!

    val queueLength: Int get() = nativeQueueLen(ptr)
    val isConnected: Boolean get() = nativeIsConnected(ptr)

    protected fun finalize() { nativeFree(ptr) }
}
```

### 3. Gradle 配置

```gradle
// app/build.gradle.kts
android {
    ndkVersion = "26.1.10909125"
    defaultConfig {
        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a")
        }
    }
}
```

将编译好的 `.so` 放到 `app/src/main/jniLibs/<abi>/libzenoh_link_state.so`。

---

## 跨平台编译脚本

```bash
#!/bin/bash
# build-mobile.sh — 一键编译所有移动平台

set -euo pipefail

PROFILE="${1:-release}"
FLAGS=""
if [ "$PROFILE" = "release" ]; then FLAGS="--release"; fi

echo "=== iOS (arm64) ==="
cargo build $FLAGS --target aarch64-apple-ios

echo "=== Android (arm64) ==="
cargo build $FLAGS --target aarch64-linux-android

echo "=== Android (armv7) ==="
cargo build $FLAGS --target armv7-linux-androideabi

echo "=== 产出物 ==="
find target -name "libzenoh_link_state.*" -type f
```

---

## 注意事项

| 项 | 说明 |
|------|------|
| 线程安全 | `LinkStateMachine` 非 `Send+Sync`，在移动端建议用 Mutex 保护 |
| 内存 | FFI 层的 `Box` 由调用方管理，`free` 必须配对调用 |
| iOS 模拟器 | 使用 `x86_64-apple-ios` target |
| Apple Silicon Mac | 可使用 `aarch64-apple-ios-sim` target |
| Android NDK | 需要 NDK r21+，推荐 r26 |
