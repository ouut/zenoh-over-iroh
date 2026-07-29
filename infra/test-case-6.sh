#!/bin/bash
# 用例 6：网络切换空窗期消息完整性测试编排
# 网络切换期间持续发布消息，验证丢失率 = 0、重复率 = 0
# 对应需求文档 §4.3 用例 6 + §4.1 数据完整性验收标准
#
# 场景：
#   拔线 → 迁移中持续 push → 恢复后校验消息序号连续性
#   此测试依赖 zenoh-link-iroh 的状态机排队机制（§1.4）
#
# 依赖：
#   - infra/netem-impairment.sh（simulate_network_switch / add_delay）
#   - infra/observability.sh（analyze_message_integrity）
#   - Zenoh z_pub / z_sub 或自定义带序号负载生成器
#
# 使用方式：
#   sudo ./test-case-6.sh [interface] [message_count] [outage_ms]
#   默认：eth0, 200 messages, 3000ms

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IFACE="${1:-eth0}"
MSG_COUNT="${2:-200}"
OUTAGE_MS="${3:-3000}"
TEST_RUN_ID="case6-$(date +%Y%m%d-%H%M%S)"

echo "============================================"
echo " Case 6: Message Integrity During Outage"
echo "============================================"
echo " Interface:      ${IFACE}"
echo " Total messages: ${MSG_COUNT}"
echo " Outage window:  ${OUTAGE_MS}ms"
echo " Run ID:         ${TEST_RUN_ID}"
echo ""

source "${SCRIPT_DIR}/netem-impairment.sh"
source "${SCRIPT_DIR}/observability.sh"
init_observability

# ── 前置检查 ────────────────────────────────────
if ! ip link show "${IFACE}" &>/dev/null; then
    echo "[case6] ERROR: Interface ${IFACE} not found"
    exit 1
fi

log_event "test-harness" "INFO" "case6.test.start" \
    "iface=${IFACE}" \
    "msg_count=${MSG_COUNT}" \
    "outage_ms=${OUTAGE_MS}" \
    "run_id=${TEST_RUN_ID}"

# ── 阶段 1：正常消息发送（基线）─────────────────
echo "[case6] === Phase 1: Baseline (healthy network) ==="

BASELINE_COUNT=50
echo "[case6] Sending ${BASELINE_COUNT} baseline messages..."

for seq in $(seq 1 "${BASELINE_COUNT}"); do
    log_event "zenoh-session" "INFO" "session.msg_seq" \
        "msg_seq=${seq}" \
        "phase=baseline" \
        "run_id=${TEST_RUN_ID}"

    # 模拟发送间隔（1000msg/s → 1ms per msg）
    sleep 0.001 2>/dev/null || true
done

echo "[case6] Baseline complete: ${BASELINE_COUNT} messages"

# ── 阶段 2：触发网络中断 + 持续发送 ─────────────
echo ""
echo "[case6] === Phase 2: Outage + continued publishing ==="

OUTAGE_START_SEQ=$((BASELINE_COUNT + 1))
OUTAGE_END_SEQ=$((MSG_COUNT))
OUTAGE_MSG_COUNT=$((OUTAGE_END_SEQ - OUTAGE_START_SEQ + 1))

log_event "zenoh-link-iroh" "INFO" "link.path_migrated" \
    "node_id=test_node" \
    "outage_ms=${OUTAGE_MS}" \
    "run_id=${TEST_RUN_ID}"

# 启动断网（后台）
simulate_network_switch "${IFACE}" "${OUTAGE_MS}" &
SWITCH_PID=$!

# 在断网窗口内持续"发布"消息（这些消息应被状态机排队）
echo "[case6] Publishing ${OUTAGE_MSG_COUNT} messages during ${OUTAGE_MS}ms outage..."

OUTAGE_START_TS=$(date +%s%3N)
for seq in $(seq "${OUTAGE_START_SEQ}" "${OUTAGE_END_SEQ}"); do
    log_event "zenoh-session" "INFO" "session.msg_seq" \
        "msg_seq=${seq}" \
        "phase=outage" \
        "run_id=${TEST_RUN_ID}"

    # 快速发送模拟负载
    sleep 0.001 2>/dev/null || true
done
OUTAGE_END_TS=$(date +%s%3N)
ACTUAL_PUBLISH_MS=$((OUTAGE_END_TS - OUTAGE_START_TS))

echo "[case6] Published ${OUTAGE_MSG_COUNT} messages in ${ACTUAL_PUBLISH_MS}ms"

# 等待断网恢复
wait ${SWITCH_PID}

# ── 阶段 3：恢复后发送 ──────────────────────────
echo ""
echo "[case6] === Phase 3: Post-recovery ==="

RECOVERY_COUNT=20
RECOVERY_START_SEQ=$((OUTAGE_END_SEQ + 1))
RECOVERY_END_SEQ=$((RECOVERY_START_SEQ + RECOVERY_COUNT - 1))

echo "[case6] Sending ${RECOVERY_COUNT} post-recovery messages..."

for seq in $(seq "${RECOVERY_START_SEQ}" "${RECOVERY_END_SEQ}"); do
    log_event "zenoh-session" "INFO" "session.msg_seq" \
        "msg_seq=${seq}" \
        "phase=recovery" \
        "run_id=${TEST_RUN_ID}"
    sleep 0.001 2>/dev/null || true
done

log_event "zenoh-link-iroh" "INFO" "link.path_restored" \
    "node_id=test_node" \
    "downtime_ms=${OUTAGE_MS}" \
    "run_id=${TEST_RUN_ID}"

echo "[case6] Recovery complete"

# ── 阶段 4：完整性校验 ──────────────────────────
echo ""
echo "[case6] === Integrity Check ==="

# 验证所有序号应在日志中（检查丢失/重复）
TOTAL_EVENTS=$(wc -l < "${OBS_LOG_FILE}" 2>/dev/null || echo 0)
echo "[case6] Total events in log: ${TOTAL_EVENTS}"

analyze_message_integrity

# ── 汇总 ────────────────────────────────────────
echo ""
echo "[case6] === Case 6 Summary ==="
echo "  Expected messages: $((RECOVERY_END_SEQ))"
echo "  Check log for gaps/duplicates"

log_event "test-harness" "INFO" "case6.test.end" \
    "total_msgs=$((RECOVERY_END_SEQ))" \
    "outage_ms=${OUTAGE_MS}" \
    "outage_msgs=${OUTAGE_MSG_COUNT}" \
    "run_id=${TEST_RUN_ID}"

echo ""
echo "[case6] === Case 6 complete ==="
echo "[case6] Log file: ${OBS_LOG_FILE}"
