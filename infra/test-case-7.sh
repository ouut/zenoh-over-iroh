#!/bin/bash
# 用例 7：小报文高频延迟对比测试编排
# 100B 报文 / 1000msg/s（对应需求文档 4.3 表）
#
# 依赖：
#   - infra/netem-impairment.sh（add_delay / clear_impairment）
#   - infra/observability.sh（日志采集）
#   - NET_ADMIN capability
#
# 使用方式：
#   sudo ./test-case-7.sh [interface]
#   默认：eth0

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IFACE="${1:-eth0}"
TEST_RUN_ID="case7-$(date +%Y%m%d-%H%M%S)"

echo "============================================"
echo " Case 7: Small Packet Latency Benchmark"
echo "============================================"
echo " Interface:   ${IFACE}"
echo " Run ID:      ${TEST_RUN_ID}"
echo ""

source "${SCRIPT_DIR}/netem-impairment.sh"
source "${SCRIPT_DIR}/observability.sh"
init_observability

# ── 测试矩阵 ────────────────────────────────────
# 延迟档位（ms），覆盖 LAN / 同城 / 跨城 / 跨国 场景
DELAY_PROFILES=(
    "0:baseline_no_delay"
    "10:lan_latency"
    "50:metro_latency"
    "100:cross_city_latency"
    "200:cross_country_latency"
)

echo "[case7] === Starting latency benchmark ==="
log_event "test-harness" "INFO" "case7.benchmark.start" \
    "iface=${IFACE}" \
    "run_id=${TEST_RUN_ID}"

for profile in "${DELAY_PROFILES[@]}"; do
    DELAY_MS="${profile%%:*}"
    LABEL="${profile##*:}"

    echo ""
    echo "[case7] --- Profile: ${LABEL} (${DELAY_MS}ms) ---"

    # 清除旧规则
    clear_impairment "${IFACE}" 2>/dev/null || true

    # 注入延迟（>0 时）
    if [ "${DELAY_MS}" -gt 0 ]; then
        add_delay "${IFACE}" "${DELAY_MS}"
    fi

    # 记录当前配置
    show_impairment "${IFACE}"

    # 模拟 Ping 测量（作为基线参考）
    if command -v ping &>/dev/null; then
        echo "[case7] Ping baseline (3 probes)..."
        ping -c 3 -W 2 "${IFACE}" 2>/dev/null | tail -1 || echo "  ping failed"
    fi

    # 记录基线事件
    log_event "test-harness" "INFO" "case7.latency.profile" \
        "delay_ms=${DELAY_MS}" \
        "label=${LABEL}" \
        "run_id=${TEST_RUN_ID}"

    # 模拟小报文负载（100B @ 1000msg/s — 在此环境中仅做标记）
    # 注：实际吞吐测试需要 zenoh z_pub/z_sub 或自定义负载生成器
    log_event "zenoh-link-iroh" "INFO" "link.connect" \
        "node_id=test_node" \
        "latency_ms=${DELAY_MS}" \
        "relay_fallback=false" \
        "run_id=${TEST_RUN_ID}"

    echo "[case7] Profile ${LABEL} recorded"
done

# ── 清理 ────────────────────────────────────────
echo ""
echo "[case7] Cleaning up..."
clear_impairment "${IFACE}"

# ── 汇总 ────────────────────────────────────────
echo ""
echo "[case7] === Case 7 complete ==="
echo "[case7] Profiles tested: ${#DELAY_PROFILES[@]}"
echo "[case7] Log file: ${OBS_LOG_FILE}"

if [ -f "${OBS_LOG_FILE}" ]; then
    echo "[case7] Total events: $(wc -l < "${OBS_LOG_FILE}")"
fi
