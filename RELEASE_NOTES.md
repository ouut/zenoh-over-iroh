# zenoh-link-state v0.1.0

First release of the three-state link state machine for **zenoh transport plugins**.

## What is this?

`zenoh-link-state` provides a connection state machine that sits between zenoh's transport layer and the underlying Iroh QUIC connection. It solves the fundamental problem: **QUIC path migration ≠ connection loss**, but zenoh's link model only knows "alive" or "dead". This state machine introduces a `Migrating` state that absorbs transient path changes without triggering unnecessary reconnects.

## Features

- **Three-state machine**: `Connected → Migrating → Connected` (transparent recovery) or `Connected → Migrating → Disconnected` (timeout)
- **Backpressure**: `with_backpressure(N)` limits queue depth during migration
- **IrohTransportLink**: ready-to-use integration layer for zenoh transport plugins
- **Async ticker**: background polling with `start_ticker(on_timeout)` callback
- **Tracing**: `link.path_migrated` / `link.path_restored` / `MigrationTimeout` events

## Quick Start

```rust
use zenoh_link_state::link_state::LinkStateMachine;

let mut sm = LinkStateMachine::new();

// Path loss → migrates, no error
sm.on_path_change(false);
assert!(sm.is_migrating());

// Data is queued, not rejected
sm.write(b"hello".to_vec()).unwrap(); // Queued

// Path restored → transparent recovery
sm.on_path_change(true);
assert!(sm.is_connected());

// Queued data is flushed
let recovered: Vec<_> = sm.drain_queue().into_iter().collect();
```

## Testing

- **33/33 tests PASS** (unit + integration + tokio async simulation)
- **9 example programs** (1-8 basic + chat room demo)
- **15 shell scripts** for NAT simulation, network impairment, observability, E2E orchestration

## TCP Baseline (localhost via zenohd REST)

| Load | Throughput |
|------|-----------|
| 100B × 200msg | 74 msg/s |
| 1MB × 10msg | 800 Mbps |

## Install

```bash
cargo add zenoh-link-state
```

Or in `Cargo.toml`:

```toml
[dependencies]
zenoh-link-state = "0.1"
```

## License

MIT OR Apache-2.0
