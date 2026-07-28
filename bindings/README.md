# Bindings

`zenoh-link-state` 的多语言绑定和跨架构编译支持。

## 目录

| 目录 | 语言 | 方案 |
|------|------|------|
| `js/`     | JavaScript / TypeScript | WebAssembly (wasm-bindgen) |
| `python/` | Python                | ctypes (免编译) |
| `lua/`    | Lua                   | C FFI via LuaJIT |
| `ios/` | iOS (Swift) | C FFI + Bridging Header |
| `android/` | Android (Kotlin) | C FFI + JNI |

## 支持的架构

| 架构 | 目标 target | 用途 |
|------|------------|------|
| x86_64 | `x86_64-unknown-linux-gnu` | Linux 服务器 |
| x86_64 | `x86_64-apple-darwin` | macOS |
| x86_64 | `x86_64-pc-windows-msvc` | Windows |
| aarch64 | `aarch64-unknown-linux-gnu` | ARM Linux (树莓派等) |
| aarch64 | `aarch64-apple-ios` | iOS 真机 |
| aarch64 | `aarch64-linux-android` | Android arm64 |
| armv7 | `armv7-linux-androideabi` | Android arm32 |
| wasm32 | `wasm32-unknown-unknown` | Web 浏览器 / Node.js |

## 一键编译

```bash
# 所有桌面 + 移动平台
./bindings/build-all.sh

# 仅 WASM
./bindings/build-all.sh --wasm

# 仅移动端
./bindings/build-all.sh --mobile
```
