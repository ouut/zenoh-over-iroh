// hello.js — zenoh-link-state JavaScript Hello World
//
// 运行前提:
//   cargo build --release --target wasm32-unknown-unknown
//   wasm-bindgen target/wasm32-unknown-unknown/release/zenoh_link_state.wasm --out-dir pkg --target node
//   node hello.js

import { zenoh_lsm_new, zenoh_lsm_new_with_backpressure,
         zenoh_lsm_free, zenoh_lsm_on_path_change, zenoh_lsm_write,
         zenoh_lsm_can_read, zenoh_lsm_tick, zenoh_lsm_drain,
         zenoh_lsm_queue_len, zenoh_lsm_is_connected,
         zenoh_lsm_is_migrating, zenoh_lsm_disconnect } from './pkg/zenoh_link_state.js';

const EVENT = { 0: "none", 1: "path_migrated", 2: "path_restored", 3: "migration_timeout" };
const STATUS = { 0: "sent", 1: "queued", 2: "backpressure", "-1": "disconnected" };

console.log("=== zenoh-link-state JavaScript Hello World ===\n");

// 1. new
let lsm = zenoh_lsm_new();
console.log(`[new]          connected=${!!zenoh_lsm_is_connected(lsm)}`);

// 2. write
let enc = new TextEncoder();
let r = zenoh_lsm_write(lsm, enc.encode("hello from JS"));
console.log(`[write]        Sent: ${STATUS[r]}`);

// 3. can_read
console.log(`[can_read]     OK: ${zenoh_lsm_can_read(lsm) === 0}`);

// 4. on_path_change
let ev = zenoh_lsm_on_path_change(lsm, 0);
console.log(`[path_change]  Migrating: ${EVENT[ev]}`);

// 5. write (Migrating → Queued)
zenoh_lsm_write(lsm, enc.encode("queued_1"));
zenoh_lsm_write(lsm, enc.encode("queued_2"));
console.log(`[write]        Queued: queue=${zenoh_lsm_queue_len(lsm)}`);

// 6. tick
ev = zenoh_lsm_tick(lsm);
console.log(`[tick]         event=${EVENT[ev]}`);

// 7. on_path_change (恢复)
ev = zenoh_lsm_on_path_change(lsm, 1);
console.log(`[path_change]  Restored: ${EVENT[ev]}`);

// 8. drain
let buf = new Uint8Array(256);
let n = zenoh_lsm_drain(lsm, buf);
console.log(`[drain]        Recovered ${n} bytes`);

// 9. backpressure
zenoh_lsm_free(lsm);
lsm = zenoh_lsm_new_with_backpressure(2);
zenoh_lsm_on_path_change(lsm, 0);
zenoh_lsm_write(lsm, enc.encode("a"));
zenoh_lsm_write(lsm, enc.encode("b"));
r = zenoh_lsm_write(lsm, enc.encode("c"));
console.log(`[backpressure] ${STATUS[r]}`);

// 10. disconnect
zenoh_lsm_disconnect(lsm);
r = zenoh_lsm_write(lsm, enc.encode("x"));
console.log(`[write]        Disconnected: ${STATUS[r]}`);

// 11. free
zenoh_lsm_free(lsm);
console.log("[free]         OK");

console.log("\n=== ALL PASS ===");
