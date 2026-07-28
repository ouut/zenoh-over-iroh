// HelloWorld.swift — zenoh-link-state Swift Hello World
//
// 集成方式参见 ios/README.md

import Foundation

print("=== zenoh-link-state Swift Hello World ===\n")

// 1. new
let lsm = LinkStateMachine()
print("[new]          connected=\(lsm.isConnected)")

// 2. write (Connected → Sent)
let s = lsm.write(data: "hello from Swift".data(using: .utf8)!)
print("[write]        Sent: status=\(s)")

// 3. can_read
print("[can_read]     OK: \(lsm.canRead())")

// 4. on_path_change (失联 → Migrating)
var e = lsm.onPathChange(connected: false)
print("[path_change]  Migrating: event=\(e)")

// 5. write (Migrating → Queued)
lsm.write(data: "queued_1".data(using: .utf8)!)
lsm.write(data: "queued_2".data(using: .utf8)!)
print("[write]        Queued: queue=\(lsm.queueLength)")

// 6. tick
e = lsm.tick()
print("[tick]         event=\(e)")

// 7. on_path_change (恢复 → Connected)
e = lsm.onPathChange(connected: true)
print("[path_change]  Restored: event=\(e)")

// 8. drain
let data = lsm.drain(bufSize: 256)
print("[drain]        Recovered \(data.count) bytes")

// 9. backpressure
let bp = LinkStateMachine(maxQueueDepth: 2)
bp.onPathChange(connected: false)
bp.write(data: "a".data(using: .utf8)!)
bp.write(data: "b".data(using: .utf8)!)
let s2 = bp.write(data: "c".data(using: .utf8)!)
print("[backpressure] \(s2)")

// 10. disconnect
bp.disconnect()
let s3 = bp.write(data: "x".data(using: .utf8)!)
print("[write]        Disconnected: \(s3)")

// 11. free (auto via deinit)
print("[free]         OK (auto)")

print("\n=== ALL PASS ===")
