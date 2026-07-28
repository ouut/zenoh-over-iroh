# Phase 3 第二批测试报告

> 生成日期：2026-07-28
> 对应需求文档 §4.3 第二批用例（2, 5, 6, 8）

---

## 1. 执行摘要

| 维度 | 目标 | 结果 | 状态 |
|------|------|------|:---:|
| 用例 2 编排 | 不对称 NAT 建链脚本 | test-case-2.sh 交付 | ✅ |
| 用例 5 编排 | IP 变化/NAT 刷新 + P50/P95/P99 | test-case-5.sh 交付 + Rust 测试 PASS | ✅ |
| 用例 6 编排 | 消息完整性（丢失率/重复率） | test-case-6.sh 交付 + Rust 测试 PASS | ✅ |
| 用例 8 编排 | 大报文吞吐对比 | test-case-8.sh 交付 + Rust 大报文测试 PASS | ✅ |
| 第二批 Rust 测试 | tokio 异步场景验证 | 4 个测试全部 PASS | ✅ |

---

## 2. 测试执行结果

```
第二批新增测试 (4):
  test_case5_repeated_nat_refresh_with_data           PASS  5周期×20msg, 2s/周期
  test_case6_message_integrity_recovery_vs_timeout    PASS  Part A: 100msg完整恢复
                                                            Part B: 50msg超时作废+重连清白
  test_case8_large_payload_queueing                   PASS  10×1MB 排队, 7.5ms排队完成
  test_sustained_write_during_long_migration          PASS  收到20条, 超时拒绝30条, 队列清空

全量测试 (29):
  单元测试: 12/12  PASS
  集成测试:  6/6   PASS
  网络模拟:  6/6   PASS  (第一批)
  第二批:    4/4   PASS
  文档测试:  1/1   PASS
```

---

## 3. 交付物清单

### 第二批新增

| 文件 | 行数 | 说明 |
|------|------|------|
| `infra/test-case-2.sh` | 105 | 不对称 NAT（对称 vs 端口限制）建链编排 |
| `infra/test-case-5.sh` | 133 | 多周期 IP 切换 + P50/P95/P99 耗时采集 |
| `infra/test-case-6.sh` | 157 | 断网空窗期消息完整性验证（4 阶段） |
| `infra/test-case-8.sh` | 155 | 大报文吞吐对比（6 种网络条件 × 1MB payload） |
| `tests/batch2_tests.rs` | 257 | 4 个 tokio 异步测试（用例 5/6/8 场景） |

### 累计交付（第一批 + 第二批）

| 类别 | 文件数 | 总行数 |
|------|:------:|:------:|
| Rust 源码 + 测试 | 5 | 1,169 |
| Shell 编排脚本 | 12 | 1,540 |
| Docker/Config | 2 | 65 |
| 文档 | 5 | ~1,500 |

---

## 4. 关键发现

### 4.1 状态机验证结论

| 场景 | 预期行为 | 实测结果 |
|------|---------|:---:|
| 短时失联（<4s）→ 恢复 | 排队数据完整，丢失率=0 | ✅ PASS |
| 超时（>4s）→ Disconnected | 排队数据作废，队列清空 | ✅ PASS |
| 重连后新连接 | 无旧数据残留，重复率=0 | ✅ PASS |
| 多次 IP 切换（5 cycles） | 每周期 20 条数据完整恢复 | ✅ PASS |
| 大报文排队（1MB × 10） | 7.5ms 完成排队，完整排出 | ✅ PASS |
| 长时间迁移中持续写入 | 超时前写入成功，超时后优雅拒绝 | ✅ PASS |

### 4.2 大报文排队性能

`test_case8_large_payload_queueing` 实测：
- 10 条 × 1MB = 10MB 数据在 7.5ms 内完成排队
- 数据完整性验证通过（标记字节校验）
- 吞吐降幅取决于下游实际发送速度，排队本身开销可忽略

---

## 5. 风险登记表更新

### 5.10 【新增】大报文排队可能产生内存压力

- **影响**：1MB × 高频消息排队可能导致 `VecDeque` 内存快速增长。当前无排队深度上限。
- **状态**：建议在集成到 `zenoh-link-iroh` 时增加 `max_queue_depth` 配置项，超限时应用背压。

---

## 6. 验收标准汇总

| 交付物要求（需求文档 4.4） | 状态 |
|------|:---:|
| 测试基础设施代码（拓扑编排、netem/iptables 规则集） | ✅ 已完成 |
| 自动化测试用例集及执行报告模板 | ✅ 已完成 |
| 性能对比报告（Zenoh TCP vs Iroh） | ⏳ 待 Docker + Zenoh/Iroh 环境 |
| 已知问题清单与风险等级评估 | ✅ 已更新（5.1–5.10） |
| 1.4 节 Migrating 超时阈值实测标定 | ⏳ 待用例 4/5 Docker 实测 |

---

## 7. 下一步（第三批）

第三批任务按编排手册需等待自建 Relay 部署：

- 用例 3：N 并发节点建链（需自建 Relay）
- 用例 9：Relay vs 直连性能基线（需自建 Relay）
- 自建 `iroh-relay` 部署方案设计
- 24h Soak Test 执行方案
- CI 特权 Runner 需求文档

当前第二批所有可执行的代码和脚本已交付。第三批依赖 Docker + 自建 Relay 环境。
