# Python Binding

通过 ctypes 直接调用编译好的 `.so` / `.dll` / `.dylib`。**无需编译任何 Python 扩展。**

## 使用

```python
from zenoh_link_state import LinkStateMachine

lsm = LinkStateMachine()

# 路径变化
event = lsm.on_path_change(False)  # → 'path_migrated'

# 写入数据
status = lsm.write(b"hello")        # → 'queued'

# 恢复
lsm.on_path_change(True)            # → 'path_restored'

# 查询
print(lsm.is_connected)  # True
print(lsm.queue_length)  # 0

# 超时检测
event = lsm.tick()  # → 'none' 或 'migration_timeout'
```

## 安装

```bash
# 1. 编译共享库
cargo build --release

# 2. 复制到 Python 项目
cp target/release/libzenoh_link_state.so my_project/

# 3. 复制 Python wrapper
cp bindings/python/zenoh_link_state.py my_project/
```
