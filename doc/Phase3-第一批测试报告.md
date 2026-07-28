# Phase 3 第一批测试报告

> 生成日期：2026-07-28
> 对应需求文档 §4.1 验收标准 + §4.3 第一批用例

---

## 1. 执行摘要

| 维度 | 目标 | 结果 | 状态 |
|------|------|------|:---:|
| 接口设计 | 1.4 节三态状态机落地 | 完整实现 + 25 测试全部 PASS | ✅ |
| 测试基础设施 | 双端对称 NAT + netem + 一键启停 | 9 个脚本/配置全部交付 | ✅ |
| 网络模拟测试 | tokio 异步时序验证 | 6 个测试全部 PASS | ✅ |
| 用例 4 编排 | 网络切换模拟脚本 | test-case-4.sh 交付 | ✅ |
| 用例 7 编排 | 小报文延迟对比脚本 | test-case-7.sh 交付 | ✅ |
| 用例 1 | 对称 NAT 建链 | 编排脚本就绪，待 Docker 环境执行 | ⏳ |

---

## 2. 验收标准逐项检查

### 2.1 连通性

| 标准 | 结果 | 说明 |
|------|:---:|------|
| 打洞直连成功率 ≥ 60% | ⏳ 待执行 | docker-compose 拓扑 + nat-simulation.sh 已就绪 |
| Relay 保底成功率 = 100% | ⏳ 待执行 | 需实际 Iroh Relay 对端 |

### 2.2 自愈性

| 标准 | 结果 | 说明 |
|------|:---:|------|
| 恢复时间 P95 ≤ 5s | ✅ 设计就绪 | 状态机超时 4s，tokio 测试验证超时机制正确 |
| 业务层无感知重复订阅 | ✅ 设计就绪 | Migrating 态不上抛 Error，恢复后自动回到 Connected |

### 2.3 数据完整性

| 标准 | 结果 | 说明 |
|------|:---:|------|
| Reliable 模式丢失率 = 0 | ✅ 设计就绪 | 排队数据在 Migrating 恢复后正确排出 |
| Reliable 模式重复率 = 0 | ✅ 设计就绪 | 超时作废队列 `queue.clear()` 防止重连误发 |

### 2.4 性能

| 标准 | 结果 | 说明 |
|------|:---:|------|
| 吞吐下降 ≤ 15% | ⏳ 待执行 | 需实际 Zenoh/Iroh 二进制运行 |
| P99 延迟增幅 ≤ 20% | ⏳ 待执行 | 需实际 Zenoh/Iroh 二进制运行 |

### 2.5 稳定性

| 标准 | 结果 | 说明 |
|------|:---:|------|
| 24h 内存增长 ≤ 5% | ⏳ 待执行 | 第三批 24h Soak Test |
| 无 fd 泄漏 | ⏳ 待执行 | 第三批 |

---

## 3. 交付物清单

### 3.1 接口设计

| 文件 | 行数 | 说明 |
|------|------|------|
| `Cargo.toml` | 9 | 最小项目配置 |
| `src/lib.rs` | 38 | 模块入口 + 文档 |
| `src/link_state.rs` | 459 | 三态状态机完整实现（含 12 个内联测试） |
| `tests/link_state_tests.rs` | 176 | 6 个集成测试 |
| `tests/network_simulation_tests.rs` | 240 | 6 个 tokio 异步时序测试 |
| `状态机设计说明.md` | 159 | mermaid 状态图 + 行为表 + 约束检查 |

### 3.2 测试基础设施

| 文件 | 行数 | 说明 |
|------|------|------|
| `infra/docker-compose.yml` | 56 | 双节点拓扑（NET_ADMIN） |
| `infra/nat-simulation.sh` | 131 | 对称/端口限制/双端对称 NAT |
| `infra/netem-impairment.sh` | 163 | 延迟/丢包/乱序/限速/断网/组合 |
| `infra/observability.sh` | 217 | JSONL 日志 + P50/P95/P99 分析 |
| `infra/start.sh` | 82 | 一键启动 + NAT 选择 |
| `infra/stop.sh` | 48 | 一键清理 + 残留检查 |
| `infra/test-case-4.sh` | 111 | 用例 4：网络切换模拟 |
| `infra/test-case-7.sh` | 100 | 用例 7：小报文延迟对比 |
| `infra/README.md` | 162 | 完整使用文档 |

---

## 4. 测试执行结果

```
运行测试套件: cargo test
结果: 25 passed, 0 failed

单元测试 (12):
  test_initial_state_is_connected       ok
  test_connected_to_migrating_and_back  ok
  test_write_queues_during_migration    ok
  test_write_in_connected_sent_immed    ok
  test_write_in_disconnected_returns_err ok
  test_read_in_disconnected_returns_err  ok
  test_read_in_connected_and_migrating  ok
  test_migration_timeout_discards_queue ok
  test_duplicate_path_change_noop       ok
  test_disconnected_ignores_path_change ok
  test_default_creates_connected        ok
  test_drain_queue_clears_internal      ok

集成测试 (6):
  test_normal_migration_cycle           ok
  test_migration_timeout_discards_data  ok
  test_repeated_migration_cycles        ok
  test_disconnected_rejects_all         ok
  test_instant_recovery_no_data_loss    ok
  test_write_connected_returns_sent     ok

网络模拟测试 (6 - tokio async):
  test_tick_timeout_enters_disconnected ok  (4.5s 实际等待)
  test_quick_recovery_within_timeout    ok  (1s 恢复)
  test_concurrent_writes_during_migr    ok  (10 并发 task)
  test_tick_polling_loop                ok  (100ms 轮询)
  test_multiple_migration_cycles        ok  (3 次反复)
  test_micro_flash_migration            ok  (50ms 微闪)

文档测试 (1):
  src/lib.rs doc-test                   ok
```

---

## 5. 风险登记表更新

### 5.8 【新增】tc netem 100% 丢包模拟断网的精度限制

- **影响**：`simulate_network_switch` 使用 tc netem 100% 丢包模拟网络中断，与真实物理断网（网卡 down）行为存在差异：TCP/QUIC 协议栈可能对 100% 丢包和链路 down 的感知延迟不同。
- **状态**：标记为已知限制，真实设备回归测试中对比校准。

### 5.9 【新增】当前环境无 Docker daemon，容器化 NAT 测试无法执行

- **影响**：用例 1（双端对称 NAT 建链）的编排脚本已就绪，但需在装有 Docker 的环境中执行。容器内无 `/var/run/docker.sock`。
- **状态**：标记为环境依赖项，需迁移到有 Docker 的环境执行。

---

## 6. Migrating 超时阈值标定

| 参数 | 当前值 | 来源 |
|------|--------|------|
| `MIGRATING_TIMEOUT_MS` | **4000ms** | 需求文档建议 3-5s 经验值（中位） |

> ⚠️ **待标定**：用例 4/5 实测数据尚未采集（需 Docker + Zenoh/Iroh 运行环境），经验值 4000ms 暂作为占位。tokio 异步测试已验证超时机制在 4500ms 时正确触发 Disconnected，4s 阈值逻辑正确。

---

## 7. 下一步

### 第二批（就绪待派发）
- 接口设计 Agent 产出已完成 → 可直接推进
- 用例 2：一端对称 NAT / 一端公网（需 Docker）
- 用例 5：IP 变化但网络未真正中断（需 Docker + Zenoh/Iroh）
- 用例 6：网络切换空窗期消息完整性（依赖用例 4 结果）
- 用例 8：大报文低频吞吐对比（需 Docker + Zenoh/Iroh）

### 第三批（待自建 Relay）
- 用例 3：N 并发节点建链
- 用例 9：Relay vs 直连性能基线
- 自建 `iroh-relay` 部署
- 24h Soak Test
