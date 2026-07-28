#!/bin/bash
# 一键启动脚本 — Zenoh × Iroh Phase 3 测试拓扑
#
# 使用方式：
#   ./start.sh                    # 默认配置
#   ./start.sh --nat symmetric    # 双端对称 NAT
#   ./start.sh --nat mixed        # 对称 vs 端口限制

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
NAT_MODE="${1:-none}"  # none | symmetric | mixed

echo "============================================"
echo " Zenoh × Iroh Phase 3 — Test Topology Start"
echo "============================================"
echo ""
echo " NAT mode: ${NAT_MODE}"
echo ""

# ── Step 1: 启动容器 ───────────────────────────
echo "[1/4] Starting Docker containers..."
cd "${SCRIPT_DIR}"
docker compose up -d

# ── Step 2: 等待容器就绪 ───────────────────────
echo "[2/4] Waiting for containers to be ready..."
MAX_WAIT=120
ELAPSED=0
while [ ${ELAPSED} -lt ${MAX_WAIT} ]; do
    READY_A=$(docker exec zenoh-test-node-a echo "ready" 2>/dev/null || echo "")
    READY_B=$(docker exec zenoh-test-node-b echo "ready" 2>/dev/null || echo "")
    if [ "${READY_A}" = "ready" ] && [ "${READY_B}" = "ready" ]; then
        echo "  Both nodes ready after ${ELAPSED}s"
        break
    fi
    sleep 2
    ELAPSED=$((ELAPSED + 2))
    echo "  Waiting... (${ELAPSED}s)"
done

if [ ${ELAPSED} -ge ${MAX_WAIT} ]; then
    echo "  ERROR: Timeout waiting for containers"
    docker compose down
    exit 1
fi

# ── Step 3: 配置 NAT 拓扑 ─────────────────────
echo "[3/4] Configuring NAT topology..."

case "${NAT_MODE}" in
    symmetric)
        echo "  Mode: Double Symmetric NAT"
        source "${SCRIPT_DIR}/nat-simulation.sh"
        setup_double_symmetric_nat
        ;;
    mixed)
        echo "  Mode: Symmetric vs Port-Restricted"
        source "${SCRIPT_DIR}/nat-simulation.sh"
        setup_symmetric_vs_port_restricted
        ;;
    none|*)
        echo "  Mode: No NAT (direct connectivity)"
        ;;
esac

# ── Step 4: 状态报告 ─────────────────────────
echo "[4/4] Topology ready!"
echo ""
echo "============================================"
echo " Container Status:"
echo "============================================"
docker ps --filter "name=zenoh-test-node" --format "  {{.Names}}  |  {{.Status}}" 2>/dev/null || true
echo ""
echo " Node A IP: $(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' zenoh-test-node-a 2>/dev/null || echo 'N/A')"
echo " Node B IP: $(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' zenoh-test-node-b 2>/dev/null || echo 'N/A')"
echo ""
echo " Useful commands:"
echo "   docker exec -it zenoh-test-node-a bash"
echo "   docker logs zenoh-test-node-a"
echo "   ./stop.sh"
echo ""
