#import <Foundation/Foundation.h>
#import <QuartzCore/QuartzCore.h>
#import <TargetConditionals.h>
#include <stdbool.h>
void cove_runtime_with_pool(void(*callback)(void*),void *context){@autoreleasepool{callback(context);}}
#if TARGET_OS_IOS
#import <UIKit/UIKit.h>
@interface CoveRuntimeDisplayDriver:NSObject
@property(nonatomic,strong)CADisplayLink *link;
@property(nonatomic)bool(*tick)(void*);
@property(nonatomic)void(*releaseContext)(void*);
@property(nonatomic)void *context;
@property(nonatomic)BOOL inCallback,stopping;
- (void)refresh:(CADisplayLink*)link;
- (void)stop;
@end
@implementation CoveRuntimeDisplayDriver
- (void)refresh:(CADisplayLink*)link{(void)link;CoveRuntimeDisplayDriver *keepAlive=self;keepAlive.inCallback=YES;BOOL running=keepAlive.tick&&keepAlive.context?keepAlive.tick(keepAlive.context):NO;keepAlive.inCallback=NO;if(!running||keepAlive.stopping)[keepAlive stop];}
- (void)stop{self.stopping=YES;[self.link invalidate];self.link=nil;if(!self.inCallback&&self.context){void *context=self.context;self.context=NULL;if(self.releaseContext)self.releaseContext(context);}}
- (void)dealloc{[self stop];}
@end
void *cove_runtime_display_driver_start(bool(*tick)(void*),void(*releaseContext)(void*),void *context){CoveRuntimeDisplayDriver *driver=[CoveRuntimeDisplayDriver new];driver.tick=tick;driver.releaseContext=releaseContext;driver.context=context;driver.link=[CADisplayLink displayLinkWithTarget:driver selector:@selector(refresh:)];[driver.link addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];return(__bridge_retained void*)driver;}
void cove_runtime_display_driver_stop(void *raw){if(!raw)return;CoveRuntimeDisplayDriver *driver=CFBridgingRelease(raw);[driver stop];}
#endif
