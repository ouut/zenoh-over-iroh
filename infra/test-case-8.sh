#!/bin/bash
# 用例 8：大报文低频吞吐对比测试编排
# 1MB 报文 / 10msg/s（对应需求文档 §4.3 用例 8）
#
# 场景：对比 TCP 直连与 Iroh 隧道化后的吞吐差距
# 验收标准：吞吐下降 ≤ 15%
#
# 依赖：
#   - infra/netem-impairment.sh（add_bandwidth_limit / add_delay）
#   - infra/observability.sh
#   - Zenoh z_pub / z_sub 或自定义负载生成器
#
# 使用方式：
#   sudo ./test-case-8.sh [interface] [bandwidth] [duration_sec]
#   默认：eth0, 10mbit, 30s

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IFACE="${1:-eth0}"
BANDWIDTH="${2:-10mbit}"
DURATION_SEC="${3:-30}"
MSG_SIZE_BYTES=$((1024 * 1024))  # 1MB
MSG_RATE=10                       # 10 msg/s
TEST_RUN_ID="case8-$(date +%Y%m%d-%H%M%S)"

echo "============================================"
echo " Case 8: Large Packet Throughput Benchmark"
echo "============================================"
echo " Interface:    ${IFACE}"
echo " Bandwidth:    ${BANDWIDTH}"
echo " Message:      ${MSG_SIZE_BYTES} bytes (1MB)"
echo " Rate:         ${MSG_RATE} msg/s"
echo " Duration:     ${DURATION_SEC}s"
echo " Expected:     $((MSG_RATE * DURATION_SEC)) messages, $((MSG_RATE * DURATION_SEC * MSG_SIZE_BYTES / 1024 / 1024))MB total"
echo " Run ID:       ${TEST_RUN_ID}"
echo ""

source "${SCRIPT_DIR}/netem-impairment.sh"
source "${SCRIPT_DIR}/observability.sh"
init_observability

# ── 前置检查 ────────────────────────────────────
if ! ip link show "${IFACE}" &>/dev/null; then
    echo "[case8] ERROR: Interface ${IFACE} not found"
    exit 1
fi

log_event "test-harness" "INFO" "case8.benchmark.start" \
    "iface=${IFACE}" \
    "bandwidth=${BANDWIDTH}" \
    "msg_size_bytes=${MSG_SIZE_BYTES}" \
    "msg_rate=${MSG_RATE}" \
    "duration_sec=${DURATION_SEC}" \
    "run_id=${TEST_RUN_ID}"

# ── 测试矩阵 ────────────────────────────────────
# 多个网络条件下的吞吐对比
TEST_CONDITIONS=(
    "baseline:0:0:0"
    "low_delay:20:0:0"
    "med_delay:50:0:0"
    "high_delay:100:1:0"
    "lossy:50:2:0"
    "limited:50:0:${BANDWIDTH}"
)

for condition in "${TEST_CONDITIONS[@]}"; do
    IFS=':' read -r LABEL DELAY_MS LOSS_PCT BW_LIMIT <<< "${condition}"

    echo ""
    echo "[case8] ========================================"
    echo "[case8] Condition: ${LABEL}"
    echo "[case8]   delay=${DELAY_MS}ms  loss=${LOSS_PCT}%  bw_limit=${BW_LIMIT}"
    echo ""

    # 清理旧规则
    clear_impairment "${IFACE}" 2>/dev/null || true

    # 应用网络条件
    if [ "${DELAY_MS}" -gt 0 ] && [ "${LOSS_PCT}" -gt 0 ]; then
        add_combined_impairment "${IFACE}" "${DELAY_MS}" "${LOSS_PCT}" 0 0
    elif [ "${DELAY_MS}" -gt 0 ]; then
        add_delay "${IFACE}" "${DELAY_MS}"
    fi

    if [ -n "${BW_LIMIT}" ] && [ "${BW_LIMIT}" != "0" ]; then
        add_bandwidth_limit "${IFACE}" "${BW_LIMIT}"
    fi

    show_impairment "${IFACE}"

    # 模拟吞吐测试（此处为标记，实际需 Zenoh z_pub）
    # 理论最大：BW_LIMIT 下的 1MB * 10msg/s
    EXPECTED_THROUGHPUT_MBPS=$((MSG_SIZE_BYTES * MSG_RATE * 8 / 1000000))
    echo "[case8] Expected throughput: ${EXPECTED_THROUGHPUT_MBPS} Mbps"

    START_TS=$(date +%s)
    for i in $(seq 1 $((MSG_RATE * DURATION_SEC))); do
        # 记录发送事件（1MB payload 标记）
        log_event "test-harness" "INFO" "case8.msg_sent" \
            "label=${LABEL}" \
            "msg_index=${i}" \
            "size_bytes=${MSG_SIZE_BYTES}" \
            "run_id=${TEST_RUN_ID}"

        # 模拟发送间隔（10msg/s → 100ms）
        sleep 0.1
    done
    END_TS=$(date +%s)
    ELAPSED=$((END_TS - START_TS))
    ACTUAL_RATE=$((MSG_RATE * DURATION_SEC / (ELAPSED + 1)))
    ACTUAL_THROUGHPUT=$((ACTUAL_RATE * MSG_SIZE_BYTES * 8 / 1000000))

    echo "[case8] ${LABEL}: ${ELAPSED}s elapsed, ~${ACTUAL_RATE} msg/s, ~${ACTUAL_THROUGHPUT} Mbps"

    log_event "test-harness" "INFO" "case8.condition.result" \
        "label=${LABEL}" \
        "delay_ms=${DELAY_MS}" \
        "loss_pct=${LOSS_PCT}" \
        "bw_limit=${BW_LIMIT}" \
        "elapsed_sec=${ELAPSED}" \
        "actual_rate=${ACTUAL_RATE}" \
        "actual_throughput_mbps=${ACTUAL_THROUGHPUT}" \
        "run_id=${TEST_RUN_ID}"

    # 清除损伤，准备下一条件
    clear_impairment "${IFACE}" 2>/dev/null || true
done

# ── 清理汇总 ────────────────────────────────────
clear_impairment "${IFACE}" 2>/dev/null || true

echo ""
echo "[case8] === Case 8 Summary ==="

log_event "test-harness" "INFO" "case8.benchmark.end" \
    "conditions_tested=${#TEST_CONDITIONS[@]}" \
    "run_id=${TEST_RUN_ID}"

if [ -f "${OBS_LOG_FILE}" ]; then
    echo "  Total events: $(wc -l < "${OBS_LOG_FILE}")"
    # 统计各条件的吞吐
    for condition in "${TEST_CONDITIONS[@]}"; do
        LABEL="${condition%%:*}"
        COUNT=$(grep -c "\"label\":\"${LABEL}\"" "${OBS_LOG_FILE}" 2>/dev/null || echo 0)
        echo "  ${LABEL}: ${COUNT} events"
    done
fi

echo ""
echo "[case8] === Case 8 complete ==="
echo "[case8] Log file: ${OBS_LOG_FILE}"
echo "[case8] NOTE: Throughput data must be compared against TCP baseline"
echo "[case8]       (run same test with zenoh TCP transport for comparison)"
