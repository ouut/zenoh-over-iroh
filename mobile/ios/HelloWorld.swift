import Foundation
let config = """
{mode:"peer", listen:{endpoints:["tcp/127.0.0.1:0"]}}
"""
let r1 = ZenohMobile.open(withConfig: config)
print("open: \(r1 == 0 ? "OK" : "FAIL")")
let r2 = ZenohMobile.putKey("hello", value: "world")
print("put: \(r2 == 0 ? "OK" : "FAIL")")
ZenohMobile.close()
print("close: OK")
