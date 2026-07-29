#!/bin/bash
# NAT 类型模拟脚本 — Zenoh × Iroh Phase 3 测试基础设施
#
# 用途：在 Docker 容器内配置 iptables 规则，模拟不同 NAT 类型组合。
# 支持：对称 NAT、端口限制锥形 NAT
#
# 使用方式（在目标容器内执行）：
#   source nat-simulation.sh
#   setup_symmetric_nat eth0
#   teardown_nat eth0

set -euo pipefail

# ── 对称 NAT（Symmetric NAT）─────────────────────
# 每个 (src_ip, src_port, dst_ip, dst_port) 分配不同外部端口。
# 实现：MASQUERADE --random，每连接独立端口映射。

setup_symmetric_nat() {
    local iface="${1:-eth0}"

    echo "[nat] Setting up symmetric NAT on ${iface}..."

    # 清除已有 NAT 规则
    iptables -t nat -F POSTROUTING 2>/dev/null || true

    # MASQUERADE --random：每个新连接随机分配源端口（模拟对称 NAT 行为）
    iptables -t nat -A POSTROUTING -o "${iface}" -j MASQUERADE --random

    # 默认 DROP FORWARD（强制经过 NAT）
    iptables -P FORWARD DROP

    echo "[nat] Symmetric NAT activated on ${iface}"
}

# ── 端口限制锥形 NAT（Port-Restricted Cone NAT）─────
# 同源 (src_ip, src_port) → 相同外部映射端口。
# 仅允许此前已发送过数据的目标 IP:Port 回包。

setup_port_restricted_nat() {
    local iface="${1:-eth0}"

    echo "[nat] Setting up port-restricted cone NAT on ${iface}..."

    iptables -t nat -F POSTROUTING 2>/dev/null || true

    # 标准 MASQUERADE（无 --random，端口尽量保持）
    iptables -t nat -A POSTROUTING -o "${iface}" -j MASQUERADE

    # 限制回包：仅允许 ESTABLISHED/RELATED
    iptables -A INPUT -m state --state ESTABLISHED,RELATED -j ACCEPT
    iptables -A INPUT -j DROP

    echo "[nat] Port-restricted cone NAT activated on ${iface}"
}

# ── 双端对称 NAT（针对容器拓扑）─────────────────
# 在 docker compose 环境中，通过 docker exec 调用。

setup_double_symmetric_nat() {
    local container_a="${1:-zenoh-test-node-a}"
    local container_b="${2:-zenoh-test-node-b}"

    echo "[nat] Setting up double symmetric NAT topology..."
    echo "[nat]   Node A (${container_a}): symmetric"
    echo "[nat]   Node B (${container_b}): symmetric"

    docker exec "${container_a}" bash -c "
        iptables -t nat -F POSTROUTING 2>/dev/null || true
        iptables -t nat -A POSTROUTING -o eth0 -j MASQUERADE --random
    "
    docker exec "${container_b}" bash -c "
        iptables -t nat -F POSTROUTING 2>/dev/null || true
        iptables -t nat -A POSTROUTING -o eth0 -j MASQUERADE --random
    "

    echo "[nat] Double symmetric NAT activated"
}

# ── 对称 vs 端口限制 组合 ────────────────────────
setup_symmetric_vs_port_restricted() {
    local container_sym="${1:-zenoh-test-node-a}"
    local container_pr="${2:-zenoh-test-node-b}"

    echo "[nat] Setting up symmetric vs port-restricted topology..."
    echo "[nat]   Node A (${container_sym}): symmetric"
    echo "[nat]   Node B (${container_pr}): port-restricted cone"

    docker exec "${container_sym}" bash -c "
        iptables -t nat -F POSTROUTING 2>/dev/null || true
        iptables -t nat -A POSTROUTING -o eth0 -j MASQUERADE --random
    "
    docker exec "${container_pr}" bash -c "
        iptables -t nat -F POSTROUTING 2>/dev/null || true
        iptables -t nat -A POSTROUTING -o eth0 -j MASQUERADE
    "

    echo "[nat] Symmetric vs port-restricted activated"
}

# ── 清理 NAT 规则 ─────────────────────────────────
teardown_nat() {
    local container="${1:-}"

    if [ -n "${container}" ]; then
        echo "[nat] Tearing down NAT on container ${container}..."
        docker exec "${container}" bash -c "
            iptables -t nat -F POSTROUTING 2>/dev/null || true
            iptables -P INPUT ACCEPT 2>/dev/null || true
            iptables -P FORWARD ACCEPT 2>/dev/null || true
            iptables -F INPUT 2>/dev/null || true
            iptables -F FORWARD 2>/dev/null || true
        "
    else
        echo "[nat] Tearing down NAT (local)..."
        iptables -t nat -F POSTROUTING 2>/dev/null || true
        iptables -P INPUT ACCEPT 2>/dev/null || true
        iptables -P FORWARD ACCEPT 2>/dev/null || true
        iptables -F INPUT 2>/dev/null || true
        iptables -F FORWARD 2>/dev/null || true
    fi

    echo "[nat] NAT rules cleared"
}

# ── 双端清理 ──────────────────────────────────────
teardown_double_nat() {
    teardown_nat "zenoh-test-node-a"
    teardown_nat "zenoh-test-node-b"
}

echo "[nat] nat-simulation.sh loaded"
