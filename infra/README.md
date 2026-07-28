# Zenoh × Iroh Phase 3 测试基础设施使用文档

> 对应需求文档 §4.2「测试环境与基础设施需求」

## 目录结构

```
infra/
├── docker-compose.yml      # 双节点拓扑编排
├── nat-simulation.sh       # NAT 类型模拟（对称/端口限制）
├── netem-impairment.sh     # 网络损伤注入（延迟/丢包/乱序/限速/断网）
├── observability.sh        # 观测埋点采集与离线分析
├── start.sh                # 一键启动
├── stop.sh                 # 一键清理
└── README.md               # 本文档
```

## 前置依赖

- **Docker** + **Docker Compose** v2+
- `iptables`（已包含在容器镜像中）
- `iproute2`（`tc` 命令，已包含在容器镜像中）
- `bc`（可选，用于百分比计算）

## 快速开始

### 1. 启动测试拓扑（无 NAT）

```bash
cd infra
chmod +x *.sh
./start.sh
```

### 2. 启动对称 NAT 拓扑

```bash
./start.sh --nat symmetric
```

### 3. 启动混合 NAT 拓扑（对称 vs 端口限制）

```bash
./start.sh --nat mixed
```

### 4. 进入容器调试

```bash
docker exec -it zenoh-test-node-a bash
docker exec -it zenoh-test-node-b bash
```

### 5. 清理环境

```bash
./stop.sh
```

## NAT 类型说明

| NAT 类型 | iptables 实现 | 使用场景 |
|----------|-------------|---------|
| 对称 NAT (Symmetric) | `MASQUERADE --random` | 用例 1：双端对称 NAT，验证 Relay fallback |
| 端口限制锥形 NAT (Port-Restricted Cone) | `MASQUERADE` + 状态过滤 | 用例 2：一端对称 / 一端公网 |

### 手动配置 NAT

```bash
# 在容器内
source /path/to/nat-simulation.sh
setup_symmetric_nat eth0
# ... 测试 ...
teardown_nat
```

## 网络损伤参数

所有损伤函数操作 `tc netem`，使用前必须确认容器有 `NET_ADMIN` capability。

| 函数 | 参数 | 示例 |
|------|------|------|
| `add_delay` | iface, ms, [jitter_ms] | `add_delay eth0 100 20` |
| `add_packet_loss` | iface, percent | `add_packet_loss eth0 5` |
| `add_reorder` | iface, percent | `add_reorder eth0 10` |
| `add_bandwidth_limit` | iface, rate | `add_bandwidth_limit eth0 1mbit` |
| `add_combined_impairment` | iface, delay, loss, reorder, [jitter] | `add_combined_impairment eth0 50 2 5 10` |
| `simulate_network_switch` | iface, down_ms | `simulate_network_switch eth0 3000` |
| `clear_impairment` | iface | `clear_impairment eth0` |
| `show_impairment` | iface | `show_impairment eth0` |

## 观测埋点

### 日志格式（JSON Lines）

每条日志一行 JSON，格式如下：

```json
{
  "timestamp": "2026-07-28T10:30:00.123Z",
  "source": "zenoh-link-iroh",
  "level": "INFO",
  "event": "link.holepunch.success",
  "fields": {
    "node_id": "node_xyz",
    "nat_type": "symmetric",
    "latency_ms": 150,
    "relay_fallback": false
  }
}
```

### 关键事件

| 事件 | 层 | 关键字段 |
|------|-----|---------|
| `link.connect` | zenoh-link-iroh | latency_ms, node_id, relay_fallback |
| `link.holepunch.success` | zenoh-link-iroh | node_id, nat_type |
| `link.holepunch.fail` | zenoh-link-iroh | node_id, nat_type |
| `link.path_migrated` | zenoh-link-iroh | node_id |
| `link.path_restored` | zenoh-link-iroh | node_id, downtime_ms |
| `link.migration_timeout` | zenoh-link-iroh | node_id, discarded_queue_len |
| `session.msg_seq` | zenoh-session | msg_seq |

### 离线分析

```bash
source observability.sh
init_observability                          # 初始化日志目录

analyze_holepunch_success_rate              # 打洞成功率
analyze_migration_latency                   # 迁移延迟 P50/P95/P99
analyze_message_integrity                   # 消息完整性（丢失/重复）
```

## 真实设备回归环境说明

> 对应风险登记表 5.1：容器环境难以真实复现运营商级对称 NAT（CGNAT）。

### 手动执行方式

1. **硬件准备**：至少 2 台设备（笔记本 + 手机），分别连接不同运营商的 4G 热点。
2. **拓扑**：设备 A（联通 4G 热点）←→ 设备 B（电信 4G 热点），均通过 Internet 连接官方 Relay。
3. **负载**：同用例 1，持续 Pub/Sub 消息流。
4. **采集**：设备端本地运行 `observability.sh` 采集 JSONL 日志，事后离线分析。
5. **对比**：将真机 NAT 环境的打洞成功率与容器模拟结果对比，校准容器测试的置信范围。

## 测试用例映射

| 用例 | 使用的 infra 脚本 | NAT 模式 |
|------|------------------|---------|
| 用例 1：对称 NAT 建链 | nat-simulation.sh + observability.sh | symmetric |
| 用例 2：对称 vs 公网 | nat-simulation.sh | mixed |
| 用例 4：网络切换 | netem-impairment.sh (simulate_network_switch) | — |
| 用例 7：小报文延迟 | netem-impairment.sh (add_delay) | — |
| 用例 8：大报文吞吐 | netem-impairment.sh (add_bandwidth_limit) | — |

## 测试工具

### zenoh-test-tools（`zenoh-tools/`）

预编译的 pub/sub 测试程序，基于 zenoh 1.9.0：

- **z_pub**: 发布带序号的测试消息
  - `z_pub <key> <count> <interval_ms> <payload_size>`
- **z_sub**: 订阅并校验消息完整性（丢失/重复检测）
  - `z_sub <key> <timeout_secs>`

### 网络隔离（`namespace-setup.sh`）

使用 Linux network namespaces 替代 Docker：

```bash
# 在有 SYS_ADMIN 权限的主机上：
sudo ./namespace-setup.sh create   # 创建 veth 隔离网络
sudo ./namespace-setup.sh destroy  # 清理
```

创建的拓扑：`10.99.0.1 <--veth--> 10.99.0.2`

### 预编译二进制

| 工具 | 版本 | 路径 |
|------|------|------|
| zenohd | 1.9.0 | `/tmp/zenoh/zenohd` |
| iroh-relay | 1.0.3 | `/tmp/iroh-relay` |

## 已知限制

1. 当前拓扑仅包含 2 个客户端节点 + 官方 Relay（presets::N0），不包含自建 Relay。
2. 容器内 `MASQUERADE --random` 的行为与真实运营商 CGNAT 不完全一致（见风险 5.1）。
3. 多节点网状拓扑（Phase 4）尚未编排。
4. Network namespace 需要 `SYS_ADMIN` capability（当前容器无此权限，需在裸机/特权容器中运行）。
