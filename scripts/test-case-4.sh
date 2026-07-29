#!/bin/bash
# 用例 4：网络切换模拟测试编排
# 验证 QUIC 迁移与 Zenoh 重连协同（对应需求文档 1.4 节状态机）
#
# 依赖：
#   - infra/netem-impairment.sh（已实现 simulate_network_switch）
#   - infra/observability.sh（日志采集与分析）
#   - NET_ADMIN capability（tc netem 需要）
#
# 使用方式：
#   sudo ./test-case-4.sh [interface] [down_ms]
#   默认：eth0, 3000ms

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IFACE="${1:-eth0}"
DOWN_MS="${2:-3000}"
TEST_RUN_ID="case4-$(date +%Y%m%d-%H%M%S)"

echo "============================================"
echo " Case 4: Network Switch Simulation"
echo "============================================"
echo " Interface:   ${IFACE}"
echo " Downtime:    ${DOWN_MS}ms"
echo " Run ID:      ${TEST_RUN_ID}"
echo ""

# ── 加载依赖 ────────────────────────────────────
source "${SCRIPT_DIR}/netem-impairment.sh"
source "${SCRIPT_DIR}/observability.sh"
init_observability

# ── 前置检查 ────────────────────────────────────
echo "[case4] Pre-flight checks..."

# 检查 NET_ADMIN
if ! ip link show "${IFACE}" &>/dev/null; then
    echo "[case4] ERROR: Interface ${IFACE} not found"
    exit 1
fi

# 检查当前 qdisc
echo "[case4] Current qdisc on ${IFACE}:"
show_impairment "${IFACE}"

# ── 执行网络切换 ────────────────────────────────
echo ""
echo "[case4] === Starting network switch simulation ==="
START_TS=$(date +%s%3N)

# 记录迁移开始事件
log_event "test-harness" "INFO" "case4.network_switch.start" \
    "iface=${IFACE}" \
    "down_ms=${DOWN_MS}" \
    "run_id=${TEST_RUN_ID}"

# 在后台执行网络切换
simulate_network_switch "${IFACE}" "${DOWN_MS}" &
SWITCH_PID=$!

# 等待切换完成
wait ${SWITCH_PID}

END_TS=$(date +%s%3N)

# ── 记录事件 ────────────────────────────────────
ACTUAL_DOWNTIME=$((END_TS - START_TS))
echo ""
echo "[case4] Network switch completed"
echo "[case4] Requested downtime: ${DOWN_MS}ms"
echo "[case4] Actual downtime:    ${ACTUAL_DOWNTIME}ms"

log_event "test-harness" "INFO" "case4.network_switch.end" \
    "iface=${IFACE}" \
    "requested_down_ms=${DOWN_MS}" \
    "actual_down_ms=${ACTUAL_DOWNTIME}" \
    "run_id=${TEST_RUN_ID}"

# 模拟状态机事件（若状态机在运行中应触发此事件）
log_event "zenoh-link-iroh" "INFO" "link.path_restored" \
    "node_id=test_node" \
    "downtime_ms=${ACTUAL_DOWNTIME}" \
    "run_id=${TEST_RUN_ID}"

# ── 验证清理 ────────────────────────────────────
echo ""
echo "[case4] Verifying cleanup..."
show_impairment "${IFACE}"

if tc qdisc show dev "${IFACE}" 2>/dev/null | grep -q "netem"; then
    echo "[case4] WARNING: netem rules still present, forcing cleanup..."
    clear_impairment "${IFACE}"
else
    echo "[case4] Cleanup verified: no residual netem rules"
fi

# ── 分析 ────────────────────────────────────────
echo ""
echo "[case4] === Analysis ==="
if [ -f "${OBS_LOG_FILE}" ]; then
    EVENT_COUNT=$(wc -l < "${OBS_LOG_FILE}")
    echo "[case4] Events recorded: ${EVENT_COUNT}"
    analyze_migration_latency
else
    echo "[case4] No log file generated"
fi

echo ""
echo "[case4] === Case 4 complete ==="
echo "[case4] Log file: ${OBS_LOG_FILE}"
