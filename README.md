# zenoh-link-iroh — Zenoh × Iroh P2P Transport Plugin

> 让 Zenoh 跑在 Iroh 的 P2P 网络上，解决复杂 NAT/移动网络下的连通性问题。

---

## 项目状态

```
Phase 3 第一批 ✅  第二批 ✅  第三批 ⏳ (自建 Relay 部署后)
33/33 tests PASS  │  15 shell scripts  │  8 docs  │  zenohd 1.9.0 + iroh-relay 1.0.3
```

## 快速开始

```bash
# 1. 运行所有测试
cargo test                                    # 33/33 PASS

# 2. 启动测试基础设施 (需要 Docker)
cd infra && ./start.sh --nat symmetric

# 3. 运行 E2E 测试编排器
./infra/run_all_tests.sh                      # 自动生成报告

# 4. 性能基准 (需要 zenohd)
/tmp/zenoh/zenohd -P rest --rest-http-port 8001 -l tcp/0.0.0.0:7447 &
./infra/zenoh-rest-bench.sh all               # TCP 基线: 74 msg/s, 800 Mbps
```

## 架构

```
Zenoh 业务层 (Pub/Sub/Query)
        │
zenoh_transport::LinkUnicastTrait
        │
┌───────┴────────┐
│ IrohTransportLink  │  ← src/iroh_integration.rs
│   ├─ write/read     │
│   ├─ tick()         │
│   └─ on_path_change │
└───────┬────────┘
        │
┌───────┴────────┐
│ LinkStateMachine   │  ← src/link_state.rs (33 tests)
│   Connected        │
│   Migrating (排队)  │
│   Disconnected     │
└───────┬────────┘
        │
   iroh::Endpoint (QUIC P2P)
```

## 目录

```
├── src/
│   ├── lib.rs                  # 模块入口
│   ├── link_state.rs           # 三态状态机 (270行, 16 tests)
│   └── iroh_integration.rs     # Zenoh 插件集成层 (370行, 2 tests)
├── tests/
│   ├── link_state_tests.rs     # 集成测试 (6 tests)
│   ├── network_simulation_tests.rs  # tokio 异步时序 (6 tests)
│   └── batch2_tests.rs         # 第二批用例 (4 tests)
├── infra/
│   ├── run_all_tests.sh        # E2E 主编排器
│   ├── namespace-setup.sh      # Network namespace 隔离
│   ├── nat-simulation.sh       # NAT 模拟
│   ├── netem-impairment.sh     # tc netem 损伤注入
│   ├── observability.sh        # JSONL 日志 + 分析
│   ├── zenoh-rest-bench.sh     # REST API 基准
│   ├── test-case-{2,4,5,6,7,8}.sh  # 用例编排
│   └── docker-compose.yml      # Docker 拓扑
├── doc/
│   ├── Zenoh-Iroh整合项目-完整需求文档.md
│   ├── Agent编排指令-Phase3执行手册.md
│   ├── 状态机设计说明.md
│   ├── 插件集成指南.md
│   ├── 自建Relay部署方案.md
│   ├── Phase4-前置设计文档.md
│   ├── Phase3-第一批测试报告.md
│   └── Phase3-第二批测试报告.md
└── Cargo.toml                  # [dependencies] tokio, tracing
```

## 核心设计

### LinkStateMachine (§1.4)

```
Connected ──(路径失联)──> Migrating ──(超时4s)──> Disconnected
                │                          ↑
                └──(路径恢复)──→ Connected ──┘
```

- `write()`: Connected → 直发 | Migrating → 排队 | Disconnected → 报错
- `tick()`: 每100ms轮询，超时后清空队列 + 上抛断连
- **背压**: `with_backpressure(N)` 限制排队深度（风险5.10）

### IrohTransportLink

封装 `LinkStateMachine` 为 Zenoh transport 友好的接口：
- `start_ticker(on_timeout)` — 后台轮询，超时时回调
- `on_path_change(bool)` — 路径恢复后自动排出排队数据
- 与 Iroh QUIC Endpoint 事件对齐

## 性能基准 (TCP localhost)

| 负载 | 吞吐 | 工具 |
|------|------|------|
| 100B × 200msg | 74 msg/s | zenohd REST API |
| 1MB × 10msg | 800 Mbps | zenohd REST API |

> ⚠️ 基于 presets::N0 官方 Relay，未做容量压测（风险5.4）
> 待 Iroh transport 就绪后补充 TCP vs Iroh 对比数据

## 风险登记

| 编号 | 风险 | 状态 |
|:---:|------|:---:|
| 5.2 | QUIC 迁移与 Zenoh 重连语义冲突 | ✅ 状态机设计已落地 |
| 5.4 | Relay 容量模型未验证 | ⏳ 待用例 3/9 |
| 5.10 | Migrating 排队无深度上限 | ✅ 背压机制已实现 |
| 5.8 | tc netem 精度限制 | ⏳ 待真机校准 |
| 5.9 | Network namespace 权限受限 | ⏳ 待裸机执行 |

详见 `doc/Zenoh-Iroh整合项目-完整需求文档.md` §5。

## 下一步

1. **Docker 环境**: `./infra/start.sh --nat symmetric` → 跑 NAT 测试
2. **自建 Relay**: 按 `doc/自建Relay部署方案.md` 部署 → 启动第三批
3. **编译插件**: 按 `doc/插件集成指南.md` 编译 cdylib → `zenohd -P iroh_link`
4. **标定**: 用例 4/5 实测 P95 迁移耗时 → 回填 `MIGRATING_TIMEOUT_MS`
