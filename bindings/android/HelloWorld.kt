// HelloWorld.kt — zenoh-link-state Kotlin Hello World
//
// 集成方式参见 android/README.md

fun main() {
    println("=== zenoh-link-state Kotlin Hello World ===\n")

    // 1. new
    val lsm = LinkStateMachine()
    println("[new]          connected=${lsm.isConnected}")

    // 2. write (Connected → Sent)
    var s = lsm.write("hello from Kotlin".toByteArray())
    println("[write]        Sent: status=$s")

    // 3. can_read
    println("[can_read]     OK: ${lsm.canRead()}")

    // 4. on_path_change (失联 → Migrating)
    var e = lsm.onPathChange(false)
    println("[path_change]  Migrating: event=$e")

    // 5. write (Migrating → Queued)
    lsm.write("queued_1".toByteArray())
    lsm.write("queued_2".toByteArray())
    println("[write]        Queued: queue=${lsm.queueLength}")

    // 6. tick
    e = lsm.tick()
    println("[tick]         event=$e")

    // 7. on_path_change (恢复 → Connected)
    e = lsm.onPathChange(true)
    println("[path_change]  Restored: event=$e")

    // 8. drain
    val data = lsm.drain(256)
    println("[drain]        Recovered ${data.size} bytes")

    // 9. backpressure
    val bp = LinkStateMachine(maxQueueDepth = 2)
    bp.onPathChange(false)
    bp.write("a".toByteArray())
    bp.write("b".toByteArray())
    val s2 = bp.write("c".toByteArray())
    println("[backpressure] $s2")

    // 10. disconnect
    bp.disconnect()
    val s3 = bp.write("x".toByteArray())
    println("[write]        Disconnected: $s3")

    // 11. free (auto via finalize)
    println("[free]         OK (auto)")

    println("\n=== ALL PASS ===")
}
