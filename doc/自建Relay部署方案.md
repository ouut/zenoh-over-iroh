# 自建 iroh-relay 部署方案

> 对应需求文档 §2 基础设施部署方案 + Phase 3 第三批 / Phase 4 前置
> 当前阶段：方案设计（待第三批触发后执行实际部署）

---

## 1. 背景

| 阶段 | 方案 | 成本 | 当前状态 |
|------|------|------|:---:|
| 开发测试 | 官方 Relay `presets::N0` | 0 | **当前正在使用** |
| 生产部署 | 自建 `iroh-relay` | 低成本（1核1G VPS） | **本文档设计** |

切换原因：
- 官方 Relay 无容量保证（风险 5.4），不能用于生产
- 用例 3（N 并发建链）和用例 9（Relay 容量压测）需要可控的 Relay 环境
- Phase 4 生产部署必须使用自建 Relay

---

## 2. iroh-relay 二进制

### 2.1 预编译二进制（推荐）

```bash
# Linux x86_64
curl -sL "https://github.com/n0-computer/iroh/releases/download/v1.0.3/iroh-relay-v1.0.3-x86_64-unknown-linux-gnu.tar.gz" \
  | tar xz -C /usr/local/bin/
chmod +x /usr/local/bin/iroh-relay
iroh-relay --version  # => iroh-relay 1.0.3
```

### 2.2 源码编译

```bash
cargo install iroh-relay
```

---

## 3. 部署配置

### 3.1 最小化部署（开发测试用）

```bash
# 启动 Relay 服务器（HTTP + HTTPS）
iroh-relay \
  --http-addr 0.0.0.0:8080 \
  --https-addr 0.0.0.0:8443 \
  --secret-key seed12345678901234567890123456789012  # 32 字节 hex
```

### 3.2 生产部署（Docker Compose）

```yaml
# docker-compose.relay.yml
version: "3.9"
services:
  iroh-relay:
    image: ghcr.io/n0-computer/iroh-relay:v1.0.3
    container_name: iroh-relay
    restart: unless-stopped
    ports:
      - "8080:8080"
      - "8443:8443"
    volumes:
      - ./relay-data:/data
    environment:
      - IROH_RELAY_HTTP_ADDR=0.0.0.0:8080
      - IROH_RELAY_HTTPS_ADDR=0.0.0.0:8443
      - IROH_RELAY_SECRET_KEY=${RELAY_SECRET_KEY}
      - IROH_RELAY_DATA_DIR=/data
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 5s
      retries: 3
```

### 3.3 容量规格建议

| 场景 | CPU | 内存 | 网络 | 预估并发连接数 |
|------|-----|------|------|:---:|
| 开发测试 | 1核 | 512MB | 10Mbps | ≤ 10 |
| 小规模生产 | 1核 | 1GB | 100Mbps | ≤ 100 |
| 中规模生产 | 2核 | 2GB | 1Gbps | ≤ 500 |
| 大规模生产 | 4核+ | 4GB+ | 10Gbps | ≤ 2000 |

> ⚠️ **注意**：上表为经验估计值，实际容量需通过用例 9（Relay 容量压测）标定。

---

## 4. 客户端配置

### 4.1 Zenoh + Iroh 集成配置

```json5
// zenoh-router.json5
{
  // 使用自建 Relay 替代官方 Relay
  transport: {
    link: {
      iroh: {
        relay_url: "https://my-relay.example.com:8443",
        // 可选：同时配置多个 Relay 做冗余
        // relay_urls: [
        //   "https://relay1.example.com:8443",
        //   "https://relay2.example.com:8443"
        // ]
      }
    }
  },
  listen: {
    endpoints: ["iroh/<node_id>"]
  }
}
```

### 4.2 环境变量方式

```bash
export IROH_RELAY_URL="https://my-relay.example.com:8443"
zenohd -c zenoh-router.json5
```

---

## 5. 安全注意事项

| 项 | 说明 |
|------|------|
| 端到端加密 | Relay **全程无法解密**业务数据（QUIC TLS 1.3 端到端） |
| Secret Key | 用于 Relay 节点身份认证，**不得泄露** |
| HTTPS | 生产环境 Relay **必须启用 HTTPS**（Let's Encrypt 免费证书） |
| 防火墙 | 仅开放 8080/8443 端口，限制来源 IP |
| 速率限制 | 生产环境建议前置 Nginx/Caddy 做速率限制 |

---

## 6. 运维检查清单

### 6.1 健康检查

```bash
# Relay 存活检查
curl -f http://localhost:8080/health

# 连接数监控
curl -s http://localhost:8080/stats | jq '.active_connections'

# 带宽使用
curl -s http://localhost:8080/stats | jq '.bytes_sent, .bytes_received'
```

### 6.2 日志监控

```bash
# 关注以下日志模式：
# - "connection refused" → Relay 过载
# - "tls handshake failed" → 证书问题
# - "relay session timeout" → 客户端断连
docker logs iroh-relay --since 1h | grep -E "ERROR|WARN"
```

---

## 7. 部署检查清单

- [ ] 预编译二进制下载 / Docker 镜像拉取
- [ ] Secret Key 生成（`openssl rand -hex 32`）
- [ ] HTTPS 证书配置（Let's Encrypt / 自签名）
- [ ] 防火墙规则配置
- [ ] 健康检查 + 监控接入
- [ ] 客户端 Relay URL 配置更新
- [ ] 连通性验证（zenohd ping 通过 iroh transport）
- [ ] 容量压测（用例 9）
- [ ] 故障恢复测试（Relay 重启后客户端自动重连）

---

## 8. 与 Phase 3 测试的对接

| 测试用例 | 依赖 Relay | 状态 |
|------|:---:|------|
| 用例 1, 2 | 官方 Relay | 编排脚本就绪 |
| 用例 3 (N 并发) | **自建 Relay** | 待部署 |
| 用例 9 (容量压测) | **自建 Relay** | 待部署 |
| 用例 4, 5 (网络切换) | 直连优先 | 编排脚本就绪 |
| 用例 7, 8 (性能对比) | 任意 Relay | TCP 基线已采集 |

自建 Relay 部署完成后，第三批测试可立即启动。
