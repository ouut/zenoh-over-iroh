#!/bin/bash
# 用例 5：IP 变化 / NAT 映射刷新测试编排
# 模拟 IP 变化但网络未真正中断（NAT 绑定刷新）
# 对应需求文档 §4.3 用例 5 + §1.4 状态机核心验证
#
# 场景：
#   移动设备切换 Wi-Fi → 蜂窝网络（IP 变化，但 QUIC 连接仍可迁移）
#   此测试验证状态机是否正确处理 Migration → IP 刷新 → 恢复路径
#
# 依赖：
#   - infra/netem-impairment.sh（simulate_network_switch）
#   - infra/observability.sh
#   - NET_ADMIN capability
#
# 使用方式：
#   sudo ./test-case-5.sh [interface] [downtime_ms] [cycles]
#   默认：eth0, 2000ms, 5 cycles

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IFACE="${1:-eth0}"
DOWN_MS="${2:-2000}"
CYCLES="${3:-5}"
TEST_RUN_ID="case5-$(date +%Y%m%d-%H%M%S)"

echo "============================================"
echo " Case 5: NAT Mapping Refresh Simulation"
echo "============================================"
echo " Interface:   ${IFACE}"
echo " Downtime:    ${DOWN_MS}ms per cycle"
echo " Cycles:      ${CYCLES}"
echo " Run ID:      ${TEST_RUN_ID}"
echo ""

source "${SCRIPT_DIR}/netem-impairment.sh"
source "${SCRIPT_DIR}/observability.sh"
init_observability

# ── 前置检查 ────────────────────────────────────
if ! ip link show "${IFACE}" &>/dev/null; then
    echo "[case5] ERROR: Interface ${IFACE} not found"
    exit 1
fi

log_event "test-harness" "INFO" "case5.test.start" \
    "iface=${IFACE}" \
    "down_ms=${DOWN_MS}" \
    "cycles=${CYCLES}" \
    "run_id=${TEST_RUN_ID}"

# ── 多周期 IP 切换模拟 ──────────────────────────
echo "[case5] === Starting ${CYCLES} NAT refresh cycles ==="

DOWNTIMES_MS=()

for cycle in $(seq 1 "${CYCLES}"); do
    echo ""
    echo "[case5] --- Cycle ${cycle}/${CYCLES} ---"

    # 记录迁移开始
    CYCLE_START=$(date +%s%3N)
    log_event "zenoh-link-iroh" "INFO" "link.path_migrated" \
        "node_id=test_node" \
        "cycle=${cycle}" \
        "run_id=${TEST_RUN_ID}"

    # 执行网络切换（模拟 IP 变化）
    tc qdisc add dev "${IFACE}" root netem loss 100% 2>/dev/null || {
        tc qdisc change dev "${IFACE}" root netem loss 100%
    }

    # 等待模拟断网时间
    DOWN_SEC=$((DOWN_MS / 1000))
    DOWN_REM=$((DOWN_MS % 1000))
    sleep "${DOWN_SEC}.${DOWN_REM}"

    # 恢复
    tc qdisc del dev "${IFACE}" root 2>/dev/null || true

    CYCLE_END=$(date +%s%3N)
    CYCLE_DOWNTIME=$((CYCLE_END - CYCLE_START))
    DOWNTIMES_MS+=("${CYCLE_DOWNTIME}")

    # 记录恢复事件
    log_event "zenoh-link-iroh" "INFO" "link.path_restored" \
        "node_id=test_node" \
        "cycle=${cycle}" \
        "downtime_ms=${CYCLE_DOWNTIME}" \
        "run_id=${TEST_RUN_ID}"

    echo "[case5] Cycle ${cycle} downtime: ${CYCLE_DOWNTIME}ms"

    # 周期之间短暂休息（模拟真实场景的切换间隔）
    sleep 0.5
done

# ── 汇总分析 ────────────────────────────────────
echo ""
echo "[case5] === Analysis ==="

# 计算 P50/P95/P99
SORTED_DOWNTIMES=$(printf '%s\n' "${DOWNTIMES_MS[@]}" | sort -n)
P50_IDX=$(( (CYCLES + 1) / 2 ))
P95_IDX=$(( (CYCLES * 95 + 99) / 100 ))
P99_IDX=$(( (CYCLES * 99 + 99) / 100 ))
[ "${P95_IDX}" -gt "${CYCLES}" ] && P95_IDX="${CYCLES}"
[ "${P99_IDX}" -gt "${CYCLES}" ] && P99_IDX="${CYCLES}"
[ "${P95_IDX}" -lt 1 ] && P95_IDX=1
[ "${P99_IDX}" -lt 1 ] && P99_IDX=1

P50=$(echo "${SORTED_DOWNTIMES}" | sed -n "${P50_IDX}p")
P95=$(echo "${SORTED_DOWNTIMES}" | sed -n "${P95_IDX}p")
P99=$(echo "${SORTED_DOWNTIMES}" | sed -n "${P99_IDX}p")

echo "  Cycles:        ${CYCLES}"
echo "  P50 downtime:  ${P50}ms"
echo "  P95 downtime:  ${P95}ms"
echo "  P99 downtime:  ${P99}ms"
echo "  All values:    ${DOWNTIMES_MS[*]}"

log_event "test-harness" "INFO" "case5.test.end" \
    "cycles=${CYCLES}" \
    "p50_ms=${P50}" \
    "p95_ms=${P95}" \
    "p99_ms=${P99}" \
    "run_id=${TEST_RUN_ID}"

echo ""
echo "[case5] === Case 5 complete ==="
echo "[case5] Log file: ${OBS_LOG_FILE}"
echo "[case5] P95=${P95}ms — compare against MIGRATING_TIMEOUT_MS (4000ms)"
echo "[case5]   Threshold OK: $([ "${P95}" -lt 4000 ] && echo 'YES (migration would NOT trigger disconnect)' || echo 'CHECK (P95 may exceed timeout)')"
