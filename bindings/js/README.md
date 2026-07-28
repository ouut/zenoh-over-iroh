# JavaScript / TypeScript Binding

通过 WebAssembly (wasm-bindgen) 将 `LinkStateMachine` 编译为 `.wasm` 供浏览器和 Node.js 使用。

## 使用

```javascript
import { LinkStateMachine, LinkEvent } from 'zenoh-link-state';

const lsm = new LinkStateMachine();

// 路径变化
lsm.onPathChange(false); // → 'path_migrated'

// 写入数据
lsm.write(new Uint8Array([0x01, 0x02])); // → 'queued'

// 恢复
lsm.onPathChange(true);  // → 'path_restored'

// 查询状态
console.log(lsm.isConnected);  // true
console.log(lsm.queueLength);  // 0
```

## 编译

```bash
# 安装 wasm 工具链
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli

# 编译
cd /path/to/zenoh-link-state
cargo build --release --target wasm32-unknown-unknown

# 生成 JS 绑定
wasm-bindgen target/wasm32-unknown-unknown/release/zenoh_link_state.wasm \
  --out-dir bindings/js/pkg \
  --target web
```
