# JavaScript / Node.js Binding

> 通过 `koffi` (C FFI) 直接调用 `libzenoh_over_iroh.so`。

## 安装

```bash
cd bindings/js
npm install koffi
```

## 使用

```javascript
const zenoh = require('./zenoh.js');

// 配置: iroh 传输层或 TCP
zenoh.open('{"listen":{"endpoints":["iroh/0.0.0.0:0"]}}');

zenoh.put('demo/test', 'hello');
console.log('ZID:', zenoh.zid());

zenoh.close();
```

## 前置条件

编译 `libzenoh_over_iroh.so`:

```bash
cargo build -p zenoh-over-iroh --release
# → target/release/libzenoh_over_iroh.so
```

## WebAssembly (浏览器)

运行时环境也可以用 WASM 编译 `zenoh-link-state` 到浏览器:

```bash
rustup target add wasm32-unknown-unknown
cargo build -p zenoh-link-state --release --target wasm32-unknown-unknown
# → target/wasm32-unknown-unknown/release/zenoh_link_state.wasm
```
但 WASM 无法使用系统网络（TCP/iroh），只能用于状态机本身的逻辑验证。
生产环境推荐 Node.js + FFI 方式。
