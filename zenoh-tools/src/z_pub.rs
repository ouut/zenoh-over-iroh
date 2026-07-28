use std::time::Duration;
use zenoh::prelude::r#async::*;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let key = args.get(1).map(|s| s.as_str()).unwrap_or("demo/test");
    let count: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(100);
    let interval_ms: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(100);
    let payload_size: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(100);

    println!("z_pub: key={key} count={count} interval={interval_ms}ms size={payload_size}B");

    let session = zenoh::open(zenoh::Config::default()).res().await.unwrap();
    let publisher = session.declare_publisher(key).res().await.unwrap();

    let payload = vec![0xABu8; payload_size];
    let start = std::time::Instant::now();

    for seq in 0..count {
        let mut data = payload.clone();
        // 前 8 字节写入序号
        let seq_bytes = seq.to_be_bytes();
        let header_len = data.len().min(8);
        data[..header_len].copy_from_slice(&seq_bytes[..header_len]);

        publisher.put(data).res().await.unwrap();

        if seq % 10 == 0 {
            let elapsed = start.elapsed().as_millis();
            let rate = if elapsed > 0 { seq as f64 / (elapsed as f64 / 1000.0) } else { 0.0 };
            println!("z_pub: seq={seq} elapsed={elapsed}ms rate={rate:.0} msg/s");
        }

        tokio::time::sleep(Duration::from_millis(interval_ms)).await;
    }

    let elapsed = start.elapsed();
    println!("z_pub: done. {count} msgs in {elapsed:?} ({:.0} msg/s)",
        count as f64 / elapsed.as_secs_f64());
    session.close().res().await.unwrap();
}
