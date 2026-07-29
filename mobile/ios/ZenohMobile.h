#ifndef ZenohMobile_h
#define ZenohMobile_h

extern int zenoh_mobile_open(const char* config);
extern int zenoh_mobile_put(const char* key, const char* value);
extern void zenoh_mobile_close(void);

@interface ZenohMobile : NSObject
+ (int)openWithConfig:(NSString*)config;
+ (int)putKey:(NSString*)key value:(NSString*)value;
+ (void)close;
@end
#endif
