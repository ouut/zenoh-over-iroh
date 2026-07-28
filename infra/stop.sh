#!/bin/bash
# 一键清理脚本 — Zenoh × Iroh Phase 3 测试拓扑
#
# 使用方式：
#   ./stop.sh          # 清理所有容器和网络

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "============================================"
echo " Zenoh × Iroh Phase 3 — Test Topology Stop"
echo "============================================"
echo ""

# ── Step 1: 清除 NAT 规则 ─────────────────────
echo "[1/3] Clearing NAT rules..."
if command -v docker &>/dev/null; then
    source "${SCRIPT_DIR}/nat-simulation.sh" 2>/dev/null && teardown_double_nat || true
fi

# ── Step 2: 清除网络损伤 ─────────────────────
echo "[2/3] Clearing network impairments..."
if command -v docker &>/dev/null; then
    for container in zenoh-test-node-a zenoh-test-node-b; do
        if docker ps -q -f "name=${container}" | grep -q .; then
            docker exec "${container}" tc qdisc del dev eth0 root 2>/dev/null || true
        fi
    done
fi

# ── Step 3: 停止并销毁容器 ───────────────────
echo "[3/3] Stopping and removing containers..."
cd "${SCRIPT_DIR}"
docker compose down -v 2>/dev/null || true

# ── 验证清理 ──────────────────────────────────
REMAINING=$(docker ps -a --filter "name=zenoh-test-node" -q 2>/dev/null | wc -l)
if [ "${REMAINING}" -eq 0 ]; then
    echo ""
    echo "============================================"
    echo " Cleanup complete: all containers removed."
    echo "============================================"
else
    echo ""
    echo " WARNING: ${REMAINING} container(s) still exist:"
    docker ps -a --filter "name=zenoh-test-node" --format "  {{.Names}}  {{.Status}}"
fi
