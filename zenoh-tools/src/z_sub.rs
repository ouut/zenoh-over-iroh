use zenoh::prelude::r#async::*;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let key = args.get(1).map(|s| s.as_str()).unwrap_or("demo/test");
    let timeout_secs: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(30);

    println!("z_sub: key={key} timeout={timeout_secs}s");

    let session = zenoh::open(zenoh::Config::default()).res().await.unwrap();
    let subscriber = session.declare_subscriber(key).res().await.unwrap();

    let start = std::time::Instant::now();
    let mut count: u64 = 0;
    let mut last_seq: Option<u64> = None;
    let mut gaps: u64 = 0;
    let mut total_bytes: u64 = 0;

    loop {
        // Check timeout
        if start.elapsed().as_secs() >= timeout_secs {
            break;
        }

        match tokio::time::timeout(
            std::time::Duration::from_secs(1),
            subscriber.recv_async(),
        )
        .await
        {
            Ok(Ok(sample)) => {
                count += 1;
                let payload = sample.payload().to_bytes();
                total_bytes += payload.len() as u64;

                // Extract sequence number (first 8 bytes)
                let mut seq_bytes = [0u8; 8];
                let header_len = payload.len().min(8);
                seq_bytes[..header_len].copy_from_slice(&payload[..header_len]);
                let seq = u64::from_be_bytes(seq_bytes);

                // Gap detection
                if let Some(prev) = last_seq {
                    if seq != prev + 1 {
                        let gap = seq.saturating_sub(prev + 1);
                        gaps += gap;
                        println!(
                            "z_sub: GAP detected seq={} prev={} gap_size={}",
                            seq, prev, gap
                        );
                    }
                }
                last_seq = Some(seq);

                if count % 10 == 0 {
                    let elapsed = start.elapsed().as_millis();
                    let rate = if elapsed > 0 {
                        count as f64 / (elapsed as f64 / 1000.0)
                    } else {
                        0.0
                    };
                    println!(
                        "z_sub: seq={seq} count={count} elapsed={elapsed}ms rate={rate:.0} msg/s gaps={gaps}"
                    );
                }
            }
            Ok(Err(e)) => {
                eprintln!("z_sub: error: {e}");
                break;
            }
            Err(_) => {
                // Timeout, loop continues
            }
        }
    }

    let elapsed = start.elapsed();
    let throughput_mbps = if elapsed.as_secs_f64() > 0.0 {
        (total_bytes as f64 * 8.0 / 1_000_000.0) / elapsed.as_secs_f64()
    } else {
        0.0
    };

    println!(
        "z_sub: done. {count} msgs, {total_bytes} bytes, {:.2} Mbps, gaps={gaps}, elapsed={elapsed:?}",
        throughput_mbps
    );
    session.close().res().await.unwrap();
}
