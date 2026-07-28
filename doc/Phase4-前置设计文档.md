# Phase 4 前置设计文档

> 对应需求文档 §3 路线图 Phase 4 + 风险 5.5/5.6/5.7

---

## 1. 多节点网格拓扑方案

### 1.1 目标

验证 zenoh-link-iroh 在 N ≥ 3 节点网状拓扑场景下的连通性与鲁棒性。
需求文档风险 5.6：点对点通过不代表网格场景无问题。

### 1.2 拓扑设计

```text
        ┌──────────────────────┐
        │   iroh-relay (自建)    │
        │   10.0.0.100:3340     │
        └──┬────────┬────────┬──┘
           │        │        │
    ┌──────┴──┐ ┌───┴────┐ ┌┴──────┐
    │ Node A  │ │ Node B │ │ Node C│  ... Node N
    │ zenohd  │ │ zenohd │ │ zenohd│
    │ + iroh  │ │ + iroh │ │ + iroh│
    └──┬───┬──┘ └───┬──┬─┘ └──┬──┬─┘
       │   └────────┼──┼──────┼──┘
       │            │  └──────┼─────┐
       └────────────┘         └─────┘
       (节点间直连打洞优先，Relay 保底)
```

### 1.3 测试场景

| 场景 | 节点数 | 操作 | 验证点 |
|------|:---:|------|------|
| 三角网格基线 | 3 | 全部互联 | 每对节点通信正常 |
| 批量加入 | 5→10→20 | 逐步扩容 | 新节点自动发现，无状态错乱 |
| 批量断连 | 20→10→5 | 随机杀节点 | 剩余节点路由收敛，无消息黑洞 |
| Relay 故障切换 | 5 | 关 Relay 5s | 直连路径不受影响，Relay 路径自动 fallback |
| 链式路由 | 5 | A→B→C→D→E | 多跳路由延迟累加，无消息丢失 |

### 1.4 编排实现

```yaml
# docker-compose.mesh.yml (5节点示例)
services:
  relay:
    image: ghcr.io/n0-computer/iroh-relay:v1.0.3
    command: --dev
    ports: ["3340:3340"]

  node-{01..05}:
    image: ubuntu:latest
    cap_add: [NET_ADMIN]
    command: >
      bash -c "
        apt-get update -qq && apt-get install -y -qq iproute2 > /dev/null 2>&1 &&
        /usr/local/bin/zenohd -c /etc/zenoh/mesh.json5 &
        sleep infinity
      "
```

### 1.5 网格收敛时间测量

| 指标 | 3节点 | 5节点 | 10节点 | 20节点 |
|------|:---:|:---:|:---:|:---:|
| 全互联时间（P95） | _待测_ | _待测_ | _待测_ | _待测_ |
| 新节点加入感知延迟 | _待测_ | _待测_ | _待测_ | _待测_ |
| 节点离开感知延迟 | _待测_ | _待测_ | _待测_ | _待测_ |

---

## 2. 24h Soak Test 方案

### 2.1 目标

验证长时间运行下的资源稳定性（需求文档 §4.1 稳定性要求）：
- 内存增长 ≤ 5%
- 无 fd 泄漏
- 无消息积压

### 2.2 测试设计

```
Duration:  24 hours
Topology:  3 nodes (triangular mesh)
Load:      1000 msg/s per node, 100B payload
Network:   每 30 分钟随机注入一次网络损伤（延迟 50ms, 丢包 1%）
Monitor:   每 10 分钟采集一次资源指标
```

### 2.3 监控指标

```bash
#!/bin/bash
# soak-monitor.sh — 每10分钟采集一次

while true; do
  TIMESTAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  
  # 内存
  for node in node-01 node-02 node-03; do
    MEM=$(docker stats --no-stream --format '{{.MemUsage}}' $node 2>/dev/null)
    echo "{\"ts\":\"$TIMESTAMP\",\"node\":\"$node\",\"mem\":\"$MEM\"}"
  done
  
  # fd 计数
  for node in node-01 node-02 node-03; do
    FD_COUNT=$(docker exec $node ls /proc/1/fd 2>/dev/null | wc -l)
    echo "{\"ts\":\"$TIMESTAMP\",\"node\":\"$node\",\"fds\":$FD_COUNT}"
  done
  
  # zenoh 内部指标
  curl -s http://localhost:8001/@/status 2>/dev/null
  
  sleep 600
done
```

### 2.4 判定标准

| 指标 | 失败阈值 | 处理 |
|------|---------|------|
| 内存增长 | > 5% over 24h | 内存泄漏排查 |
| fd 泄漏 | > 0 增长 | fd 未关闭分析 |
| 消息积压 | queue_depth > 0 持续 > 60s | 背压问题 |
| 断连 | 任何非预期的 Disconnected 事件 | 状态机 bug |
| 消息丢失 | gaps > 0 | 完整性 bug |

---

## 3. CI 特权 Runner 方案

### 3.1 问题

需求文档风险 5.5：网络损伤类测试需要 `NET_ADMIN` capability，标准 CI Runner 无此权限。

### 3.2 方案

```
┌──────────────────────────────────────────────┐
│              GitLab CI / GitHub Actions       │
│  ┌──────────┐  ┌──────────┐  ┌────────────┐  │
│  │ Standard │  │ Standard │  │ Privileged  │  │
│  │ Runner   │  │ Runner   │  │ Runner      │  │
│  │ (单元测试)│  │ (lint)   │  │ (网络模拟)  │  │
│  └──────────┘  └──────────┘  └─────┬───────┘  │
│                                     │          │
│  ┌──────────────────────────────────┘          │
│  │ Docker-in-Docker + NET_ADMIN + SYS_ADMIN    │
│  │ 专用裸金属 VM / 特权容器                     │
│  └─────────────────────────────────────────────┘
└──────────────────────────────────────────────┘
```

### 3.3 Runner 规格

| 需求 | 规格 |
|------|------|
| 实例类型 | 裸金属 VM / 特权 Docker 容器 |
| CPU | 4核+ |
| 内存 | 8GB+ |
| 磁盘 | 50GB+ SSD |
| Capabilities | NET_ADMIN, SYS_ADMIN, SYS_PTRACE |
| Docker | Docker-in-Docker (dind) 或 socket mount |
| 标签 | `network-sim`, `privileged` |

### 3.4 Pipeline 示例

```yaml
# .gitlab-ci.yml
stages:
  - build
  - unit-test
  - integration-test
  - network-sim
  - soak-test

unit-test:
  stage: unit-test
  script: cargo test --lib

integration-test:
  stage: integration-test
  script: cargo test --test '*'

network-sim:
  stage: network-sim
  tags: [privileged, network-sim]
  script:
    - cd infra && ./start.sh --nat symmetric
    - ./test-case-1.sh
    - ./test-case-4.sh
    - ./stop.sh

soak-test:
  stage: soak-test
  tags: [privileged, network-sim]
  when: manual  # 手动触发
  script: ./soak-monitor.sh 86400
```

---

## 4. Migrating 超时阈值标定计划

### 4.1 当前状态

| 参数 | 值 | 来源 |
|------|-----|------|
| `MIGRATING_TIMEOUT_MS` | 4000ms | 经验值 |
| tokio 测试验证 | PASS | 4.5s 超时触发正常 |

### 4.2 标定流程

1. 在 Docker 环境中启动 2 节点 zenohd + iroh transport
2. 用例 4：注入网络切换（tc netem 100% 丢包），N=100 次
3. 用例 5：注入 NAT 映射刷新（veth IP 变化），N=100 次
4. 采集每次迁移恢复耗时：`downtime_ms = path_restored.ts - path_migrated.ts`
5. 计算 P50 / P95 / P99
6. 标定值 = P95 × 1.3（安全裕度）
7. 回填 `MIGRATING_TIMEOUT_MS`

### 4.3 预期范围

```
P50:  ~500ms   (大部分 QUIC 迁移在 1s 内完成)
P95:  ~2000ms  (网络波动较大时)
P99:  ~3500ms  (极端情况)
标定: 2600ms   (2000 × 1.3, 明显短于当前 4000ms)
```

> 上述为估计值，实际以实测为准。若 P95 > 4000ms，需调整安全裕度倍数。
