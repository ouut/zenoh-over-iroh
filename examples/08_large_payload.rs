//! 示例: 大报文排队性能
//!
//! 演示 1MB 大报文的排队与恢复，验证数据完整性。
//! 测试 10MB 总排队量的内存和时间开销。
//!
//! 运行: cargo run --example 08_large_payload

use zenoh_link_state::link_state::{LinkStateMachine, WriteStatus};

fn main() {
    let mut sm = LinkStateMachine::new();
    let payload_size = 1024 * 1024; // 1MB
    let count = 10;
    let total = count * payload_size;

    println!("[init] 1MB × {} messages = {}MB total", count, count);

    // 进入 Migrating
    sm.on_path_change(false);

    // 排队大报文
    let large_payload = vec![0x42u8; payload_size];
    let start = std::time::Instant::now();

    for i in 0..count {
        let mut data = large_payload.clone();
        data[0] = i as u8; // 序列标记
        data[payload_size - 1] = 0xFF; // 尾部标记
        assert_eq!(sm.write(data), Ok(WriteStatus::Queued));
    }

    let queue_time = start.elapsed();
    assert_eq!(sm.queue_len(), count);

    println!(
        "[queue] {}MB queued in {:?} ({:.1} MB/s)",
        count,
        queue_time,
        total as f64 / 1_000_000.0 / queue_time.as_secs_f64()
    );

    // 恢复并逐条校验
    sm.on_path_change(true);
    let drained: Vec<_> = sm.drain_queue().into_iter().collect();
    assert_eq!(drained.len(), count);

    for (i, data) in drained.iter().enumerate() {
        assert_eq!(data.len(), payload_size, "msg {}: size mismatch", i);
        assert_eq!(data[0], i as u8, "msg {}: header corruption", i);
        assert_eq!(data[payload_size - 1], 0xFF, "msg {}: tail corruption", i);
    }

    println!("[verify] All {} messages intact ✓", count);

    // 带背压的大报文
    println!();
    println!("[backpressure] Testing with limit=3...");
    let mut bp_sm = LinkStateMachine::with_backpressure(3);
    bp_sm.on_path_change(false);

    for _i in 0..3 {
        assert_eq!(
            bp_sm.write(large_payload.clone()),
            Ok(WriteStatus::Queued)
        );
    }
    assert_eq!(
        bp_sm.write(large_payload.clone()),
        Ok(WriteStatus::Backpressure)
    );
    println!("[backpressure] 4th 1MB write → Backpressure ✓");

    println!();
    println!("=== 08_large_payload: PASS ===");
}
