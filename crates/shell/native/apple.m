#import <Foundation/Foundation.h>
#import <QuartzCore/QuartzCore.h>
#import <TargetConditionals.h>
#import <SDL3/SDL.h>
#import <dispatch/dispatch.h>
#if TARGET_OS_OSX
#import <AppKit/AppKit.h>
#import <CoreVideo/CoreVideo.h>
#import <CoreServices/CoreServices.h>
#else
#import <UIKit/UIKit.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>
#import <objc/runtime.h>
#endif
#include "native.h"
#include <stdlib.h>
#include <string.h>
@interface CoveShellAppleWindow : NSObject
@property(nonatomic,weak) id window;
@property(nonatomic,strong) CAMetalLayer *layer;
@property(nonatomic,strong) NSLock *inboxLock;
@property(nonatomic,strong) CADisplayLink *displayLink;
@property(nonatomic) BOOL enabled,hasTick;
@property(nonatomic) double timestamp,target;
@property(nonatomic) uint32_t windowID;
@property(nonatomic) CoveShellChrome chrome;
@property(nonatomic,strong) NSData *drag,*nonDrag;
#if TARGET_OS_OSX
@property(nonatomic,strong) NSMutableArray *observers;
@property(nonatomic,strong) id scrollMonitor,doubleClickMonitor;
@property(nonatomic,strong) NSVisualEffectView *backdrop;
@property(nonatomic) CVDisplayLinkRef videoLink;
@property(nonatomic,strong) NSMutableArray *signalSources;
#endif
- (void)displayDidRefresh:(CADisplayLink *)link;
- (void)publish:(double)timestamp target:(double)target;
- (void)stop;
#if TARGET_OS_OSX
- (void)alignTrafficLights;
#endif
@end
static NSMutableDictionary<NSNumber*,CoveShellAppleWindow*> *windows;
static uint32_t scrollType;
/* Documents the OS opened with the app, kept apart from a drag onto a window.
   AppKit/UIKit enqueue on the main thread; the shell drains on its own thread. */
static NSMutableArray<NSString*> *openedDocuments;
static NSLock *openedDocumentsLock;
static void cove_open_documents_push(NSString *text){
    if(!text.length)return;
    if(!openedDocumentsLock){openedDocumentsLock=[NSLock new];openedDocuments=[NSMutableArray array];}
    [openedDocumentsLock lock];[openedDocuments addObject:text];[openedDocumentsLock unlock];
    cove_shell_wake();
}
bool cove_shell_open_documents_take(char **out){
    if(!out)return false;*out=NULL;
    if(!openedDocumentsLock)return false;
    [openedDocumentsLock lock];
    NSString *text=openedDocuments.firstObject;
    if(text)[openedDocuments removeObjectAtIndex:0];
    [openedDocumentsLock unlock];
    if(!text)return false;
    *out=strdup(text.UTF8String);
    return *out!=NULL;
}
#if TARGET_OS_OSX
@interface CoveShellOpenDocumentsTarget : NSObject
- (void)handleOpenDocuments:(NSAppleEventDescriptor*)event withReplyEvent:(NSAppleEventDescriptor*)reply;
- (void)handleURLEvent:(NSAppleEventDescriptor*)event withReplyEvent:(NSAppleEventDescriptor*)reply;
@end
/* NSAppleEventManager holds the target unretained, so keep a strong reference. */
static CoveShellOpenDocumentsTarget *openDocumentsTarget;
@implementation CoveShellOpenDocumentsTarget
- (void)pushDescriptor:(NSAppleEventDescriptor*)desc{
    NSURL *url=desc.fileURLValue;
    if(!url)url=[desc coerceToDescriptorType:typeFileURL].fileURLValue;
    cove_open_documents_push(url?url.path:desc.stringValue);
}
- (void)handleOpenDocuments:(NSAppleEventDescriptor*)event withReplyEvent:(NSAppleEventDescriptor*)reply{
    (void)reply;NSAppleEventDescriptor *direct=[event paramDescriptorForKeyword:keyDirectObject];
    NSInteger count=direct.numberOfItems;
    if(count>0){for(NSInteger i=1;i<=count;i++){[self pushDescriptor:[direct descriptorAtIndex:i]];}}
    else [self pushDescriptor:direct];
}
- (void)handleURLEvent:(NSAppleEventDescriptor*)event withReplyEvent:(NSAppleEventDescriptor*)reply{
    (void)reply;cove_open_documents_push([[event paramDescriptorForKeyword:keyDirectObject] stringValue]);
}
@end
void cove_shell_open_documents_install(void){COVE_ASSERT_MAIN();
    if(openDocumentsTarget)return;
    openDocumentsTarget=[CoveShellOpenDocumentsTarget new];
    NSAppleEventManager *manager=[NSAppleEventManager sharedAppleEventManager];
    /* Installing after SDL replaces its GetURL handler, and the open-documents
       handler intercepts before AppKit would call application:openFile:. */
    [manager setEventHandler:openDocumentsTarget andSelector:@selector(handleURLEvent:withReplyEvent:) forEventClass:kInternetEventClass andEventID:kAEGetURL];
    [manager setEventHandler:openDocumentsTarget andSelector:@selector(handleOpenDocuments:withReplyEvent:) forEventClass:kCoreEventClass andEventID:kAEOpenDocuments];
}
#else
/* SDL's UIKit delegates forward openURL as a drag; divert handleURL: into the
   open-documents inbox so an OS open is not mistaken for a drop on a window. */
static void cove_open_documents_swizzle(Class cls){
    if(!cls)return;
    Method original=class_getInstanceMethod(cls,sel_registerName("handleURL:"));
    if(!original)return;
    SEL replacement=sel_registerName("cove_handleURL:");
    if(class_getInstanceMethod(cls,replacement))return;
    IMP imp=imp_implementationWithBlock(^(id self,NSURL *url){
        (void)self;NSURL *fileURL=url.filePathURL;
        cove_open_documents_push(fileURL?fileURL.path:url.absoluteString);
    });
    class_addMethod(cls,replacement,imp,"v@:@");
    method_exchangeImplementations(original,class_getInstanceMethod(cls,replacement));
}
void cove_shell_open_documents_install(void){COVE_ASSERT_MAIN();
    static dispatch_once_t once;
    dispatch_once(&once,^{
        cove_open_documents_swizzle(NSClassFromString(@"SDLUIKitSceneDelegate"));
        cove_open_documents_swizzle(NSClassFromString(@"SDLUIKitDelegate"));
    });
}
#endif
#if !TARGET_OS_OSX
/* UIKit posts the keyboard frame in points. The observers run on the main
   queue, the same thread that reads the cached inset and wakes the SDL loop,
   so a plain float is sufficient. */
static float keyboardHeight;
static id keyboardObservers[2];
static void cove_keyboard_changed(NSNotification *note) {
    NSValue *frame=note.userInfo[UIKeyboardFrameEndUserInfoKey];
    BOOL showing=[note.name isEqualToString:UIKeyboardWillShowNotification];
    keyboardHeight=showing&&frame?(float)[frame CGRectValue].size.height:0.0f;
    cove_shell_wake();
}
#endif
static int phase_value(unsigned long value) {
#if TARGET_OS_OSX
    if(value&NSEventPhaseCancelled)return 6;if(value&NSEventPhaseEnded)return 5;if(value&NSEventPhaseBegan)return 2;if(value&NSEventPhaseChanged)return 3;if(value&NSEventPhaseStationary)return 4;if(value&NSEventPhaseMayBegin)return 1;
#else
    (void)value;
#endif
    return 0;
}
@implementation CoveShellAppleWindow
- (instancetype)init { if((self=[super init])) { _inboxLock=[NSLock new];
#if TARGET_OS_OSX
    _observers=[NSMutableArray new];_signalSources=[NSMutableArray new];
#endif
} return self; }
- (void)publish:(double)timestamp target:(double)target {
    [_inboxLock lock];BOOL wake=_enabled&&!_hasTick;if(_enabled){_timestamp=timestamp;_target=target;_hasTick=YES;}[_inboxLock unlock];if(wake)cove_shell_wake();
}
- (void)displayDidRefresh:(CADisplayLink *)link {[self publish:link.timestamp target:link.targetTimestamp];}
#if TARGET_OS_OSX
- (void)alignTrafficLights {
    NSWindow *window=self.window;float height=self.chrome.title_height;if(!window||height<=0||window.styleMask&NSWindowStyleMaskFullScreen)return;
    NSButton *close=[window standardWindowButton:NSWindowCloseButton];NSView *container=close.superview.superview;if(!container)return;
    NSRect frame=container.frame;frame.size.height=height;frame.origin.y=window.frame.size.height-height;container.frame=frame;
    for(NSNumber *kind in @[@(NSWindowCloseButton),@(NSWindowMiniaturizeButton),@(NSWindowZoomButton)]){NSButton *button=[window standardWindowButton:kind.integerValue];if(button){NSPoint p=button.frame.origin;p.y=round((button.superview.frame.size.height-button.frame.size.height)/2);[button setFrameOrigin:p];}}
}
#endif
- (void)stop {
    [_inboxLock lock];_enabled=NO;_hasTick=NO;[_inboxLock unlock];[_displayLink invalidate];_displayLink=nil;
#if TARGET_OS_OSX
    if(_videoLink){CVDisplayLinkStop(_videoLink);CVDisplayLinkRelease(_videoLink);_videoLink=NULL;}
    for(id observer in _observers)[NSNotificationCenter.defaultCenter removeObserver:observer];[_observers removeAllObjects];
    if(_scrollMonitor)[NSEvent removeMonitor:_scrollMonitor];_scrollMonitor=nil;if(_doubleClickMonitor)[NSEvent removeMonitor:_doubleClickMonitor];_doubleClickMonitor=nil;
    for(dispatch_source_t source in _signalSources)dispatch_source_cancel(source);[_signalSources removeAllObjects];
#endif
}
@end
void cove_shell_apple_prepare(bool dock) {COVE_ASSERT_MAIN();
#if TARGET_OS_OSX
    [NSApplication.sharedApplication setActivationPolicy:dock?NSApplicationActivationPolicyRegular:NSApplicationActivationPolicyAccessory];
#else
    (void)dock;
#endif
}
void *cove_shell_apple_attach(void *rawWindow,void *rawLayer,bool inset,bool dock) {COVE_ASSERT_MAIN();
    cove_shell_apple_prepare(dock);CoveShellAppleWindow *context=[CoveShellAppleWindow new];context.window=(__bridge id)rawWindow;context.layer=(__bridge CAMetalLayer*)rawLayer;
    context.layer.pixelFormat=MTLPixelFormatBGRA8Unorm;context.layer.framebufferOnly=YES;
    if(!windows)windows=[NSMutableDictionary new];
#if TARGET_OS_OSX
    NSWindow *window=context.window;
    if(inset){window.titleVisibility=NSWindowTitleHidden;window.titlebarAppearsTransparent=YES;window.styleMask|=NSWindowStyleMaskFullSizeContentView;window.toolbarStyle=NSWindowToolbarStyleUnifiedCompact;}
    [window makeKeyAndOrderFront:nil];[NSApp activateIgnoringOtherApps:YES];
    if(!scrollType)scrollType=SDL_RegisterEvents(1);
    __weak CoveShellAppleWindow *weak=context;
    context.scrollMonitor=[NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskScrollWheel handler:^NSEvent*(NSEvent *event){
        CoveShellAppleWindow *self=weak;NSWindow *window=self.window;if(!self||!window||event.window!=window)return event;
        NSView *view=window.contentView;NSPoint point=[view convertPoint:event.locationInWindow fromView:nil];
        CoveShellEvent *wheel=calloc(1,sizeof(*wheel));if(!wheel)return event;
        wheel->kind=19;wheel->window=self.windowID;wheel->timestamp=event.timestamp;wheel->dx=event.scrollingDeltaX;wheel->dy=event.scrollingDeltaY;wheel->x=point.x;wheel->y=view.isFlipped?point.y:view.bounds.size.height-point.y;
        wheel->a=phase_value(event.phase);wheel->b=phase_value(event.momentumPhase);wheel->flags=event.hasPreciseScrollingDeltas?4:0;
        SDL_Event marker;SDL_zero(marker);marker.type=scrollType;marker.user.windowID=self.windowID;marker.user.data1=wheel;if(!SDL_PushEvent(&marker))free(wheel);return event;
    }];
    context.doubleClickMonitor=[NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskLeftMouseDown handler:^NSEvent*(NSEvent *event){
        CoveShellAppleWindow *self=weak;NSWindow *window=self.window;if(!self||event.window!=window||event.clickCount<2)return event;
        CoveShellChrome chrome=self.chrome;NSPoint p=event.locationInWindow;float x=p.x,y=window.contentView.bounds.size.height-p.y;BOOL drag=NO;
        for(size_t i=0;i<chrome.non_drag_count;i++){CoveShellRect r=chrome.non_drag[i];if(x>=r.x&&y>=r.y&&x<r.x+r.width&&y<r.y+r.height)return event;}
        if(chrome.explicit_drag){for(size_t i=0;i<chrome.drag_count;i++){CoveShellRect r=chrome.drag[i];if(x>=r.x&&y>=r.y&&x<r.x+r.width&&y<r.y+r.height)drag=YES;}}
        else drag=y>=0&&y<chrome.title_height&&x>=chrome.leading&&x<window.contentView.bounds.size.width-chrome.trailing;
        if(drag){[window performZoom:nil];return nil;}return event;
    }];
    for(NSNotificationName name in @[NSWindowDidResizeNotification,NSWindowDidEndLiveResizeNotification,NSWindowDidExitFullScreenNotification,NSWindowDidBecomeKeyNotification,NSWindowDidBecomeMainNotification]){
        id observer=[NSNotificationCenter.defaultCenter addObserverForName:name object:window queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *notification){(void)notification;[weak alignTrafficLights];}];[context.observers addObject:observer];
    }
    for(NSNumber *signal in @[@(SIGINT),@(SIGTERM)]){dispatch_source_t source=dispatch_source_create(DISPATCH_SOURCE_TYPE_SIGNAL,signal.intValue,0,dispatch_get_global_queue(QOS_CLASS_USER_INITIATED,0));if(source){dispatch_source_set_event_handler(source,^{cove_shell_wake();});dispatch_resume(source);[context.signalSources addObject:source];}}
#else
    (void)inset;
    if(!keyboardObservers[0]){
        keyboardObservers[0]=[NSNotificationCenter.defaultCenter addObserverForName:UIKeyboardWillShowNotification object:nil queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note){cove_keyboard_changed(note);}];
        keyboardObservers[1]=[NSNotificationCenter.defaultCenter addObserverForName:UIKeyboardWillHideNotification object:nil queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note){cove_keyboard_changed(note);}];
    }
#endif
    return (__bridge_retained void*)context;
}
void cove_shell_apple_detach(void *raw) {COVE_ASSERT_MAIN(); if(!raw)return;CoveShellAppleWindow *context=CFBridgingRelease(raw);[context stop];[windows removeObjectForKey:@(context.windowID)]; }
bool cove_shell_apple_backdrop(void *raw,int kind) {COVE_ASSERT_MAIN();
    CoveShellAppleWindow *context=(__bridge CoveShellAppleWindow*)raw;if(!context)return false;
#if TARGET_OS_OSX
    NSWindow *window=context.window;NSView *content=window.contentView;if(!window||!content)return false;
    if(kind==0){[context.backdrop removeFromSuperview];context.backdrop=nil;window.opaque=YES;window.backgroundColor=NSColor.windowBackgroundColor;context.layer.opaque=YES;return true;}
    if(!context.backdrop){
        /* Behind-window blending samples the desktop under the window; the Metal
           layer above it composites premultiplied, so clear pixels reveal it. */
        NSVisualEffectView *effect=[[NSVisualEffectView alloc] initWithFrame:content.bounds];
        effect.autoresizingMask=NSViewWidthSizable|NSViewHeightSizable;
        effect.blendingMode=NSVisualEffectBlendingModeBehindWindow;
        /* Stays translucent while another window is key; following the active
           state turns the material opaque whenever the app is in the background. */
        effect.state=NSVisualEffectStateActive;
        [content addSubview:effect positioned:NSWindowBelow relativeTo:nil];
        context.backdrop=effect;
    }
    context.backdrop.material=NSVisualEffectMaterialSidebar;
    window.opaque=NO;window.backgroundColor=NSColor.clearColor;context.layer.opaque=NO;
    /* The material reaches the top edge: content runs under a transparent
       title bar, and the bar's height is reported as the top safe area so the
       UI starts below it while the system keeps dragging and the title. */
    window.styleMask|=NSWindowStyleMaskFullSizeContentView;window.titlebarAppearsTransparent=YES;
    /* The frame is unchanged, so AppKit posts no resize; SDL learns the taller
       content area from an explicit one. */
    [NSNotificationCenter.defaultCenter postNotificationName:NSWindowDidResizeNotification object:window];
    return true;
#else
    (void)kind;return false;
#endif
}
void cove_shell_apple_appearance(void *raw,bool dark) {COVE_ASSERT_MAIN();
    CoveShellAppleWindow *context=(__bridge CoveShellAppleWindow*)raw;if(!context)return;
#if TARGET_OS_OSX
    NSWindow *window=context.window;NSAppearanceName name=dark?NSAppearanceNameDarkAqua:NSAppearanceNameAqua;
    if(![window.appearance.name isEqualToString:name])window.appearance=[NSAppearance appearanceNamed:name];
#else
    (void)dark;
#endif
}
void cove_shell_apple_update(void *raw,CoveShellWindowState *state) {COVE_ASSERT_MAIN();
    CoveShellAppleWindow *context=(__bridge CoveShellAppleWindow*)raw;if(!context)return;context.windowID=state->id;windows[@(state->id)]=context;
    state->native_window=(uintptr_t)(__bridge void*)context.window;
#if TARGET_OS_OSX
    NSWindow *window=context.window;state->visible=window.isVisible&&!NSApp.isHidden;state->maximized=window.isZoomed||(window.styleMask&NSWindowStyleMaskFullScreen);state->minimized=window.isMiniaturized;state->focused=window.isKeyWindow;state->refresh_rate=window.screen.maximumFramesPerSecond;
    if(context.backdrop&&window.contentView){NSRect layout=window.contentLayoutRect;state->top=fmax(state->top,window.contentView.bounds.size.height-NSMaxY(layout));}
#endif
    context.layer.contentsScale=fmax(1,(double)state->pixel_width/fmax(1,state->width));context.layer.drawableSize=CGSizeMake(state->pixel_width,state->pixel_height);
}
bool cove_shell_apple_command(void *raw,int command,int a) {COVE_ASSERT_MAIN();
#if TARGET_OS_OSX
    NSWindow *window=((__bridge CoveShellAppleWindow*)raw).window;if(!window)return false;
    switch(command){case 3:case 8:[window makeKeyAndOrderFront:nil];[NSApp activateIgnoringOtherApps:YES];return true;case 4:[window performMiniaturize:nil];return true;case 5:if(!window.isZoomed)[window performZoom:nil];return true;case 6:if(window.isMiniaturized)[window deminiaturize:nil];else if(window.isZoomed)[window performZoom:nil];else if(window.styleMask&NSWindowStyleMaskFullScreen)[window toggleFullScreen:nil];return true;case 7:[window orderOut:nil];return true;case 10:{BOOL on=(window.styleMask&NSWindowStyleMaskFullScreen)!=0;if(on!=(a!=0)){window.collectionBehavior|=NSWindowCollectionBehaviorFullScreenPrimary;[window toggleFullScreen:nil];}return true;}case 11:{NSWindowCollectionBehavior floating=NSWindowCollectionBehaviorCanJoinAllSpaces|NSWindowCollectionBehaviorFullScreenAuxiliary;window.level=a?NSFloatingWindowLevel:NSNormalWindowLevel;window.collectionBehavior=a?((window.collectionBehavior&~NSWindowCollectionBehaviorFullScreenPrimary)|floating):(window.collectionBehavior&~floating);return true;}default:return false;}
#else
    (void)raw;(void)command;(void)a;return false;
#endif
}
int cove_shell_apple_fullscreen(void *raw) {COVE_ASSERT_MAIN();
#if TARGET_OS_OSX
    NSWindow *window=((__bridge CoveShellAppleWindow*)raw).window;if(!window)return -1;return (window.styleMask&NSWindowStyleMaskFullScreen)!=0;
#else
    (void)raw;return -1;
#endif
}
void cove_shell_apple_chrome(void *raw,const CoveShellChrome *chrome) {COVE_ASSERT_MAIN();
    CoveShellAppleWindow *context=(__bridge CoveShellAppleWindow*)raw;if(!context)return;
    context.drag=[NSData dataWithBytes:chrome->drag length:chrome->drag_count*sizeof(CoveShellRect)];context.nonDrag=[NSData dataWithBytes:chrome->non_drag length:chrome->non_drag_count*sizeof(CoveShellRect)];
    CoveShellChrome owned=*chrome;owned.drag=context.drag.bytes;owned.non_drag=context.nonDrag.bytes;context.chrome=owned;
#if TARGET_OS_OSX
    [context alignTrafficLights];
#endif
}
bool cove_shell_apple_refresh(void *raw,bool active) {COVE_ASSERT_MAIN();
    CoveShellAppleWindow *context=(__bridge CoveShellAppleWindow*)raw;if(!context)return false;
#if TARGET_OS_OSX
    NSWindow *window=context.window;active=active&&window.isVisible&&!NSApp.isHidden&&!window.isMiniaturized&&(window.occlusionState&NSWindowOcclusionStateVisible);
    if(!context.displayLink && !context.videoLink){
        if(@available(macOS 14.0,*)){context.displayLink=[window displayLinkWithTarget:context selector:@selector(displayDidRefresh:)];[context.displayLink addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];}
        else {NSNumber *number=window.screen.deviceDescription[@"NSScreenNumber"];CVDisplayLinkRef link=NULL;if(!number||CVDisplayLinkCreateWithCGDisplay(number.unsignedIntValue,&link)!=kCVReturnSuccess)return false;
            __weak CoveShellAppleWindow *weak=context;double frequency=CVGetHostClockFrequency();CVReturn status=CVDisplayLinkSetOutputHandler(link,^CVReturn(CVDisplayLinkRef displayLink,const CVTimeStamp *now,const CVTimeStamp *output,CVOptionFlags input,CVOptionFlags *flags){(void)displayLink;(void)input;(void)flags;[weak publish:now->hostTime/frequency target:output->hostTime/frequency];return kCVReturnSuccess;});if(status!=kCVReturnSuccess){CVDisplayLinkRelease(link);return false;}context.videoLink=link;}
    }
    if(context.videoLink){if(active){if(!CVDisplayLinkIsRunning(context.videoLink)&&CVDisplayLinkStart(context.videoLink)!=kCVReturnSuccess)return false;}else CVDisplayLinkStop(context.videoLink);}
#else
    UIWindow *window=context.window;active=active&&!window.hidden&&window.windowScene.activationState==UISceneActivationStateForegroundActive;
    if(!context.displayLink){context.displayLink=[CADisplayLink displayLinkWithTarget:context selector:@selector(displayDidRefresh:)];[context.displayLink addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];}
#endif
    [context.inboxLock lock];context.enabled=active;if(!active)context.hasTick=NO;[context.inboxLock unlock];context.displayLink.paused=!active;return active;
}
bool cove_shell_apple_refresh_take(void *raw,double *timestamp,double *target) {COVE_ASSERT_MAIN();
    CoveShellAppleWindow *context=(__bridge CoveShellAppleWindow*)raw;if(!context)return false;[context.inboxLock lock];BOOL result=context.hasTick;if(result){*timestamp=context.timestamp;*target=context.target;context.hasTick=NO;}[context.inboxLock unlock];return result;
}
bool cove_shell_apple_event(CoveShellEvent *event) {COVE_ASSERT_MAIN();
#if TARGET_OS_OSX
    if(event->kind!=scrollType)return false;CoveShellEvent *wheel=(void*)(uintptr_t)event->identity;if(!wheel)return false;*event=*wheel;free(wheel);return true;
#else
    (void)event;return false;
#endif
}
bool cove_shell_apple_wheel_owned(uint32_t id) {COVE_ASSERT_MAIN();
#if TARGET_OS_OSX
    return windows[@(id)]!=nil;
#else
    (void)id;return false;
#endif
}
bool cove_shell_apple_shortcut(uint32_t id,int scancode,uint16_t modifiers) {COVE_ASSERT_MAIN();
#if TARGET_OS_OSX
    NSWindow *window=windows[@(id)].window;if(!window||!(modifiers&192))return false;switch(scancode){case 16:[window performMiniaturize:nil];return true;case 26:[window performClose:nil];return true;case 9:if(modifiers&12){[window toggleFullScreen:nil];return true;}default:break;}
#else
    (void)id;(void)scancode;(void)modifiers;
#endif
    return false;
}
float cove_shell_apple_keyboard_height(void) {COVE_ASSERT_MAIN();
#if TARGET_OS_OSX
    return 0.0f;
#else
    return keyboardHeight;
#endif
}
void cove_shell_apple_flush(void) {COVE_ASSERT_MAIN();[CATransaction flush];}
#if TARGET_OS_OSX
@interface CoveShellMenuTarget : NSObject
@property(nonatomic) BOOL chosen;
@property(nonatomic) NSInteger chosenID;
- (void)choose:(NSMenuItem*)sender;
@end
@implementation CoveShellMenuTarget
- (void)choose:(NSMenuItem*)sender{self.chosen=YES;self.chosenID=sender.tag;}
@end
/* Builds a parent-indexed snapshot into `root`. Action items route to `target`;
   submenus keep a nil action so AppKit opens them. */
static void cove_menu_append_items(NSMenu *root,const CoveShellMenuItem *items,size_t count,id target,SEL action){
    root.autoenablesItems=NO;
    NSMutableArray<NSMenuItem*> *built=[NSMutableArray arrayWithCapacity:count];
    for(size_t i=0;i<count;i++){
        CoveShellMenuItem item=items[i];NSMenuItem *entry;
        if(item.kind==1)entry=NSMenuItem.separatorItem;
        else{entry=[[NSMenuItem alloc]initWithTitle:[NSString stringWithUTF8String:item.title]?:@"" action:item.kind==0?action:nil keyEquivalent:[NSString stringWithUTF8String:item.key]?:@""];
            entry.enabled=(item.flags&1)!=0;entry.state=(item.flags&2)?NSControlStateValueOn:NSControlStateValueOff;
            if(item.kind==0){entry.target=target;entry.tag=(NSInteger)item.id;NSEventModifierFlags flags=0;if(item.flags&4)flags|=NSEventModifierFlagCommand;if(item.flags&8)flags|=NSEventModifierFlagControl;if(item.flags&16)flags|=NSEventModifierFlagOption;if(item.flags&32)flags|=NSEventModifierFlagShift;entry.keyEquivalentModifierMask=flags;}
            if(item.kind==2){entry.submenu=[[NSMenu alloc]initWithTitle:entry.title];entry.submenu.autoenablesItems=NO;}
            if(item.png&&item.png_size){NSImage *icon=[[NSImage alloc]initWithData:[NSData dataWithBytes:item.png length:item.png_size]];icon.size=NSMakeSize(16,16);icon.template=(item.flags&64)!=0;entry.image=icon;}}
        NSMenu *parent=item.parent==SIZE_MAX?root:item.parent<built.count?built[item.parent].submenu:nil;if(parent)[parent addItem:entry];[built addObject:entry];
    }
}
bool cove_shell_menu_popup(const CoveShellMenuItem *items,size_t count,float x,float y,uintptr_t rawWindow,int64_t *chosen){COVE_ASSERT_MAIN();
    CoveShellMenuTarget *target=[CoveShellMenuTarget new];NSMenu *root=[NSMenu new];
    cove_menu_append_items(root,items,count,target,@selector(choose:));
    NSWindow *window=rawWindow?(__bridge NSWindow*)(void*)rawWindow:NSApp.keyWindow;NSView *view=window.contentView;
    NSPoint point=view?NSMakePoint(x,view.isFlipped?y:view.bounds.size.height-y):NSEvent.mouseLocation;
    [root popUpMenuPositioningItem:nil atLocation:point inView:view];if(target.chosen)*chosen=target.chosenID;return target.chosen;
}

/* AppKit menu actions run on the main thread, while the authored closures live
   on the UI thread. The choice is queued here and drained by the shell's
   deferred callbacks, the same shape a native file dialog result takes. */
static NSMutableArray<NSNumber*> *menuBarChoices;
static NSLock *menuBarLock;
@interface CoveShellMenuBarTarget : NSObject
- (void)choose:(NSMenuItem*)sender;
@end
/* NSMenuItem.target is unretained, so the bar keeps its own strong reference. */
static CoveShellMenuBarTarget *menuBarTarget;
@implementation CoveShellMenuBarTarget
- (void)choose:(NSMenuItem*)sender{
    if(!menuBarLock)return;
    [menuBarLock lock];[menuBarChoices addObject:@(sender.tag)];[menuBarLock unlock];
    cove_shell_wake();
}
@end
static NSString *cove_menu_bar_string(const char *text,NSString *fallback){
    if(!text)return fallback;NSString *value=[NSString stringWithUTF8String:text];return value.length?value:fallback;
}
/* A platform entry: nil target so the selector travels the responder chain,
   which is what lets Cut/Copy/Paste reach a save panel's name field. */
static NSMenuItem *cove_menu_bar_entry(NSString *title,SEL action,NSString *key,NSEventModifierFlags mask){
    NSMenuItem *entry=[[NSMenuItem alloc]initWithTitle:title action:action keyEquivalent:key];
    entry.keyEquivalentModifierMask=mask;return entry;
}
bool cove_shell_menu_bar_install(const CoveShellMenuBar *bar){COVE_ASSERT_MAIN();
    if(!bar)return false;
    if(!menuBarLock){menuBarLock=[NSLock new];menuBarChoices=[NSMutableArray new];}
    [menuBarLock lock];[menuBarChoices removeAllObjects];[menuBarLock unlock];
    if(!menuBarTarget)menuBarTarget=[CoveShellMenuBarTarget new];
    NSString *appName=cove_menu_bar_string(bar->app_name,NSProcessInfo.processInfo.processName);
    NSMenu *mainMenu=[NSMenu new];mainMenu.autoenablesItems=NO;

    NSMenuItem *appItem=[NSMenuItem new];appItem.title=appName;
    NSMenu *appMenu=[NSMenu new];appMenu.title=appName;
    cove_menu_append_items(appMenu,bar->application_items,bar->application_count,menuBarTarget,@selector(choose:));
    if(appMenu.numberOfItems>0)[appMenu addItem:NSMenuItem.separatorItem];
    NSMenuItem *servicesItem=[NSMenuItem new];servicesItem.title=cove_menu_bar_string(bar->services,@"Services");
    servicesItem.submenu=[NSMenu new];[appMenu addItem:servicesItem];NSApp.servicesMenu=servicesItem.submenu;
    [appMenu addItem:NSMenuItem.separatorItem];
    [appMenu addItem:cove_menu_bar_entry(cove_menu_bar_string(bar->hide,[NSString stringWithFormat:@"Hide %@",appName]),@selector(hide:),@"h",NSEventModifierFlagCommand)];
    [appMenu addItem:cove_menu_bar_entry(cove_menu_bar_string(bar->hide_others,@"Hide Others"),@selector(hideOtherApplications:),@"h",NSEventModifierFlagCommand|NSEventModifierFlagOption)];
    [appMenu addItem:cove_menu_bar_entry(cove_menu_bar_string(bar->show_all,@"Show All"),@selector(unhideAllApplications:),@"",0)];
    [appMenu addItem:NSMenuItem.separatorItem];
    [appMenu addItem:cove_menu_bar_entry(cove_menu_bar_string(bar->quit,[NSString stringWithFormat:@"Quit %@",appName]),@selector(terminate:),@"q",NSEventModifierFlagCommand)];
    appItem.submenu=appMenu;[mainMenu addItem:appItem];

    NSMenu *authored=[NSMenu new];
    cove_menu_append_items(authored,bar->menus,bar->menu_count,menuBarTarget,@selector(choose:));
    NSString *editTitle=cove_menu_bar_string(bar->edit,@"Edit"),*windowTitle=cove_menu_bar_string(bar->window,@"Window");
    BOOL hasEdit=NO,hasWindow=NO;
    /* An item belongs to one menu at a time: AppKit throws when an item still
       owned by `authored` is added to the bar, so detach them first. */
    NSArray<NSMenuItem*> *entries=[authored.itemArray copy];[authored removeAllItems];
    for(NSMenuItem *entry in entries){[mainMenu addItem:entry];if([entry.title isEqualToString:editTitle])hasEdit=YES;if([entry.title isEqualToString:windowTitle])hasWindow=YES;}

    if(!hasEdit){
        NSMenuItem *editItem=[NSMenuItem new];editItem.title=editTitle;
        NSMenu *editMenu=[NSMenu new];editMenu.title=editTitle;
        /* Autoenabled: AppKit asks the responder whether undo/cut/copy/paste
           currently do anything, which a stored flag cannot answer. */
        editMenu.autoenablesItems=YES;
        [editMenu addItem:cove_menu_bar_entry(cove_menu_bar_string(bar->undo,@"Undo"),@selector(undo:),@"z",NSEventModifierFlagCommand)];
        [editMenu addItem:cove_menu_bar_entry(cove_menu_bar_string(bar->redo,@"Redo"),@selector(redo:),@"z",NSEventModifierFlagCommand|NSEventModifierFlagShift)];
        [editMenu addItem:NSMenuItem.separatorItem];
        [editMenu addItem:cove_menu_bar_entry(cove_menu_bar_string(bar->cut,@"Cut"),@selector(cut:),@"x",NSEventModifierFlagCommand)];
        [editMenu addItem:cove_menu_bar_entry(cove_menu_bar_string(bar->copy,@"Copy"),@selector(copy:),@"c",NSEventModifierFlagCommand)];
        [editMenu addItem:cove_menu_bar_entry(cove_menu_bar_string(bar->paste,@"Paste"),@selector(paste:),@"v",NSEventModifierFlagCommand)];
        [editMenu addItem:cove_menu_bar_entry(cove_menu_bar_string(bar->delete_item,@"Delete"),@selector(delete:),@"",0)];
        [editMenu addItem:cove_menu_bar_entry(cove_menu_bar_string(bar->select_all,@"Select All"),@selector(selectAll:),@"a",NSEventModifierFlagCommand)];
        editItem.submenu=editMenu;[mainMenu addItem:editItem];
    }
    if(!hasWindow){
        NSMenuItem *windowItem=[NSMenuItem new];windowItem.title=windowTitle;
        NSMenu *windowMenu=[NSMenu new];windowMenu.title=windowTitle;
        [windowMenu addItem:cove_menu_bar_entry(cove_menu_bar_string(bar->minimize,@"Minimize"),@selector(performMiniaturize:),@"m",NSEventModifierFlagCommand)];
        [windowMenu addItem:cove_menu_bar_entry(cove_menu_bar_string(bar->zoom,@"Zoom"),@selector(performZoom:),@"",0)];
        [windowMenu addItem:NSMenuItem.separatorItem];
        [windowMenu addItem:cove_menu_bar_entry(cove_menu_bar_string(bar->bring_all_to_front,@"Bring All to Front"),@selector(arrangeInFront:),@"",0)];
        windowItem.submenu=windowMenu;[mainMenu addItem:windowItem];
        /* AppKit appends the open-window list and keeps it current. */
        NSApp.windowsMenu=windowMenu;
    }
    NSApp.mainMenu=mainMenu;
    return true;
}
bool cove_shell_menu_bar_take_choice(int64_t *chosen){
    if(!menuBarLock||!chosen)return false;
    [menuBarLock lock];
    NSNumber *first=menuBarChoices.firstObject;
    if(first)[menuBarChoices removeObjectAtIndex:0];
    [menuBarLock unlock];
    if(!first)return false;*chosen=first.longLongValue;return true;
}
void cove_shell_menu_bar_clear(void){COVE_ASSERT_MAIN();
    if(menuBarLock){[menuBarLock lock];[menuBarChoices removeAllObjects];[menuBarLock unlock];}
    NSApp.servicesMenu=nil;NSApp.windowsMenu=nil;NSApp.mainMenu=nil;
}
/* Screen-colour sampling is implemented in Rust with objc2-app-kit's
   NSColorSampler binding; see crate::color_sampler::appkit. */
bool cove_shell_apple_picker(void *context,int mode,const char *path,const char *const *names,const char *const *patterns,size_t count,bool multiple,CoveShellDialogCallback callback,void *box){(void)context;(void)mode;(void)path;(void)names;(void)patterns;(void)count;(void)multiple;(void)callback;(void)box;return false;}
bool cove_shell_apple_export(void *context,const char *name,const uint8_t *bytes,size_t size,const char *const *patterns,size_t count,CoveShellDialogCallback callback,void *box){(void)context;(void)name;(void)bytes;(void)size;(void)patterns;(void)count;(void)callback;(void)box;return false;}
#else
bool cove_shell_menu_bar_install(const CoveShellMenuBar *bar){(void)bar;return false;}
bool cove_shell_menu_bar_take_choice(int64_t *chosen){(void)chosen;return false;}
void cove_shell_menu_bar_clear(void){}
/* SDL's iOS dialog backend is a stub, so Cove presents UIDocumentPicker itself.
   The delegate is retained here until UIKit calls back, because the picker only
   holds it weakly. */
@interface CoveShellDocumentPicker : NSObject <UIDocumentPickerDelegate>
@property(nonatomic) CoveShellDialogCallback callback;
@property(nonatomic) void *box;
@property(nonatomic) BOOL export;
@property(nonatomic,strong) NSURL *temporary;
@end
static NSMutableArray<CoveShellDocumentPicker*> *activePickers;
@implementation CoveShellDocumentPicker
- (void)finish:(NSArray<NSURL*>*)urls status:(int)status{
    CoveShellDialogCallback callback=self.callback;void *box=self.box;
    NSMutableArray<NSURL*>*paths=urls;
    if(self.temporary){[[NSFileManager defaultManager] removeItemAtURL:self.temporary error:nil];}
    [activePickers removeObject:self];
    if(status==0&&paths.count){
        const char **list=calloc(paths.count+1,sizeof(*list));
        for(NSUInteger i=0;i<paths.count;i++){list[i]=strdup(paths[i].path.UTF8String);}
        callback(box,list,status);
        for(NSUInteger i=0;i<paths.count;i++){free((void*)list[i]);}
        free(list);
    }else{
        const char *empty[]={NULL};
        callback(box,status==3?NULL:empty,status);
    }
}
- (void)documentPicker:(UIDocumentPickerViewController*)controller didPickDocumentsAtURLs:(NSArray<NSURL*>*)urls{
    (void)controller;[self finish:urls status:self.export?3:0];
}
- (void)documentPickerWasCancelled:(UIDocumentPickerViewController*)controller{
    (void)controller;[self finish:nil status:1];
}
@end
static NSArray<UTType*> *picker_types(const char *const *patterns,size_t count){
    NSMutableArray<UTType*> *types=[NSMutableArray array];
    for(size_t i=0;i<count;i++){
        if(!patterns[i])continue;
        NSString *joined=[NSString stringWithUTF8String:patterns[i]];
        for(NSString *ext in [joined componentsSeparatedByString:@";"]){
            NSString *trimmed=[ext stringByTrimmingCharactersInSet:[NSCharacterSet whitespaceCharacterSet]];
            if(!trimmed.length)continue;
            UTType *type=[UTType typeWithFilenameExtension:trimmed];
            if(type&&![types containsObject:type])[types addObject:type];
        }
    }
    return types;
}
static UIViewController *picker_presenter(void *raw){
    CoveShellAppleWindow *context=(__bridge CoveShellAppleWindow*)raw;
    UIWindow *window=context.window;
    UIViewController *presenter=window.rootViewController;
    while(presenter.presentedViewController)presenter=presenter.presentedViewController;
    return presenter;
}
bool cove_shell_apple_picker(void *raw,int mode,const char *path,const char *const *names,const char *const *patterns,size_t count,bool multiple,CoveShellDialogCallback callback,void *box){COVE_ASSERT_MAIN();
    (void)path;(void)names;
    UIViewController *presenter=picker_presenter(raw);if(!presenter)return false;
    NSArray<UTType*> *types=mode==1?@[[UTType folder]]:picker_types(patterns,count);
    if(!types.count)types=@[[UTType item]];
    UIDocumentPickerViewController *picker=[[UIDocumentPickerViewController alloc] initForOpeningContentTypes:types asCopy:YES];
    picker.allowsMultipleSelection=multiple;
    CoveShellDocumentPicker *delegate=[CoveShellDocumentPicker new];delegate.callback=callback;delegate.box=box;delegate.export=NO;
    picker.delegate=delegate;
    if(!activePickers)activePickers=[NSMutableArray array];
    [activePickers addObject:delegate];
    [presenter presentViewController:picker animated:YES completion:nil];
    return true;
}
bool cove_shell_apple_export(void *raw,const char *name,const uint8_t *bytes,size_t size,const char *const *patterns,size_t count,CoveShellDialogCallback callback,void *box){COVE_ASSERT_MAIN();
    (void)patterns;(void)count;
    UIViewController *presenter=picker_presenter(raw);if(!presenter)return false;
    NSString *filename=name&&*name?[NSString stringWithUTF8String:name]:@"Untitled";
    NSURL *directory=[NSURL fileURLWithPath:NSTemporaryDirectory() isDirectory:YES];
    NSURL *temporary=[directory URLByAppendingPathComponent:[NSString stringWithFormat:@"%@-%@",NSUUID.UUID.UUIDString,filename.lastPathComponent]];
    NSData *data=[NSData dataWithBytes:bytes length:size];
    if(![data writeToURL:temporary options:NSDataWritingAtomic error:nil]){callback(box,NULL,2);return true;}
    UIDocumentPickerViewController *picker=[[UIDocumentPickerViewController alloc] initForExportingURLs:@[temporary] asCopy:YES];
    CoveShellDocumentPicker *delegate=[CoveShellDocumentPicker new];delegate.callback=callback;delegate.box=box;delegate.export=YES;delegate.temporary=temporary;
    picker.delegate=delegate;
    if(!activePickers)activePickers=[NSMutableArray array];
    [activePickers addObject:delegate];
    [presenter presentViewController:picker animated:YES completion:nil];
    return true;
}
#endif
