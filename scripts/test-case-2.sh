#!/bin/bash
# 用例 2：不对称 NAT 建链测试编排
# 一端对称 NAT / 一端端口限制锥形 NAT，验证优先直连
# 对应需求文档 §4.3 用例 2
#
# 依赖：
#   - infra/nat-simulation.sh（setup_symmetric_vs_port_restricted）
#   - infra/observability.sh（日志采集与分析）
#   - Docker（运行双节点容器拓扑）
#
# 使用方式：
#   ./test-case-2.sh [duration_sec]
#   默认：运行 60s

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DURATION_SEC="${1:-60}"
TEST_RUN_ID="case2-$(date +%Y%m%d-%H%M%S)"

echo "============================================"
echo " Case 2: Asymmetric NAT Connectivity Test"
echo "============================================"
echo " Topology:   Symmetric NAT vs Port-Restricted"
echo " Duration:   ${DURATION_SEC}s"
echo " Run ID:     ${TEST_RUN_ID}"
echo ""

source "${SCRIPT_DIR}/nat-simulation.sh"
source "${SCRIPT_DIR}/observability.sh"
init_observability

# ── 检查 Docker ─────────────────────────────────
if ! command -v docker &>/dev/null; then
    echo "[case2] ERROR: Docker not available"
    echo "[case2] This test requires Docker for containerized NAT isolation."
    echo "[case2] Run './start.sh --nat mixed' first on a Docker-capable host."
    exit 1
fi

# ── 记录测试开始 ────────────────────────────────
log_event "test-harness" "INFO" "case2.test.start" \
    "topology=symmetric_vs_port_restricted" \
    "duration_sec=${DURATION_SEC}" \
    "run_id=${TEST_RUN_ID}"

# ── 配置不对称 NAT ──────────────────────────────
echo "[case2] Configuring asymmetric NAT..."
setup_symmetric_vs_port_restricted "zenoh-test-node-a" "zenoh-test-node-b"

# ── 记录 NAT 配置 ───────────────────────────────
echo "[case2] Node A (symmetric NAT):"
docker exec zenoh-test-node-a iptables -t nat -L POSTROUTING -v 2>/dev/null || true

echo "[case2] Node B (port-restricted NAT):"
docker exec zenoh-test-node-b iptables -t nat -L POSTROUTING -v 2>/dev/null || true

# ── 运行测试窗口 ────────────────────────────────
echo ""
echo "[case2] Test window: ${DURATION_SEC}s"
echo "[case2] === To complete this test, run Iroh/Zenoh on both nodes ==="
echo "[case2]   Node A (symmetric):  zenoh-link-iroh listen iroh/<node_a_id>"
echo "[case2]   Node B (port-restr): zenoh-link-iroh connect iroh/<node_a_id>"
echo ""

# 等待测试窗口
START_TS=$(date +%s)
ATTEMPT_COUNT=0
DIRECT_SUCCESS=0
RELAY_FALLBACK=0

while [ $(($(date +%s) - START_TS)) -lt "${DURATION_SEC}" ]; do
    ATTEMPT_COUNT=$((ATTEMPT_COUNT + 1))
    ELAPSED=$(($(date +%s) - START_TS))

    # 每 5s 报告一次状态
    if [ $((ELAPSED % 5)) -eq 0 ]; then
        echo "[case2] t=${ELAPSED}s | attempts=${ATTEMPT_COUNT} | direct=${DIRECT_SUCCESS} | relay=${RELAY_FALLBACK}"
    fi

    # 注：实际连接尝试由外部 Zenoh/Iroh 进程发起
    # 此处仅记录时序标记
    sleep 1
done

# ── 记录测试结束 ────────────────────────────────
log_event "test-harness" "INFO" "case2.test.end" \
    "total_attempts=${ATTEMPT_COUNT}" \
    "direct_success=${DIRECT_SUCCESS}" \
    "relay_fallback=${RELAY_FALLBACK}" \
    "run_id=${TEST_RUN_ID}"

# ── 分析 ────────────────────────────────────────
echo ""
echo "[case2] === Case 2 Summary ==="
echo "  Total attempts:    ${ATTEMPT_COUNT}"

if [ -f "${OBS_LOG_FILE}" ]; then
    echo "  Events recorded:   $(wc -l < "${OBS_LOG_FILE}")"
    analyze_holepunch_success_rate
fi

echo ""
echo "[case2] === Case 2 complete ==="
echo "[case2] Log file: ${OBS_LOG_FILE}"
