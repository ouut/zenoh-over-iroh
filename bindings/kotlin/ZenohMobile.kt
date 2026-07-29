package zenoh.mobile
class ZenohMobile {
    companion object {
        init { System.loadLibrary("zenoh_mobile") }
        @JvmStatic external fun open(config: String): Int
        @JvmStatic external fun put(key: String, value: String): Int
        @JvmStatic external fun close(): Int
    }
}
