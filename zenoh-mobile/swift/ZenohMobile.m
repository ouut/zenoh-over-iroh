#import "ZenohMobile.h"
@implementation ZenohMobile
+ (int)openWithConfig:(NSString*)c { return zenoh_mobile_open([c UTF8String]); }
+ (int)putKey:(NSString*)k value:(NSString*)v { return zenoh_mobile_put([k UTF8String], [v UTF8String]); }
+ (void)close { zenoh_mobile_close(); }
@end
