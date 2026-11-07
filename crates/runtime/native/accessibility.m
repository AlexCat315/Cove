#import <Foundation/Foundation.h>
#import <TargetConditionals.h>
#if TARGET_OS_OSX
#import <AppKit/AppKit.h>
#else
#import <UIKit/UIKit.h>
#endif
#include <stdbool.h>
#include <stdint.h>
typedef struct{uint64_t id,parent;uint32_t role,flags,actions;float x,y,width,height;const char *label,*value,*help,*identifier;uint64_t revision;} CoveAXItem;
typedef bool(*CoveAXAction)(void*,uint64_t,uint32_t,const char*);
typedef char*(*CoveAXValue)(void*,uint64_t);
typedef void(*CoveAXFreeValue)(char*);
@class CoveAXBridge;
#if TARGET_OS_OSX
@interface CoveAXElement:NSAccessibilityElement
#else
@interface CoveAXElement:UIAccessibilityElement
#endif
@property(nonatomic,weak)CoveAXBridge *bridge;
@property(nonatomic)uint64_t identity;
@property(nonatomic)uint32_t flags,actions,role;
@property(nonatomic,strong)NSDictionary *presentation;
@property(nonatomic,strong)NSArray<NSNumber*> *childIDs;
@property(nonatomic)BOOL updating;
- (BOOL)invoke:(uint32_t)action value:(NSString*)value;
- (void)invalidate;
@end
@interface CoveAXBridge:NSObject
#if TARGET_OS_OSX
@property(nonatomic,weak)NSView *view;
#else
@property(nonatomic,weak)UIView *view;
#endif
@property(nonatomic,strong)NSArray *originalChildren;
@property(nonatomic)BOOL originalIsElement;
@property(nonatomic,strong)NSMutableDictionary<NSNumber*,CoveAXElement*> *elements;
@property(nonatomic,strong)NSArray<NSNumber*> *rootIDs;
@property(nonatomic)void *context;
@property(nonatomic)CoveAXAction action;
@property(nonatomic)CoveAXValue value;
@property(nonatomic)CoveAXFreeValue freeValue;
@end
@implementation CoveAXBridge
@end
static NSString *text(const char *value){return value?[NSString stringWithUTF8String:value]?:@"":@"";}
static id resolved_value(CoveAXElement *element){
    CoveAXBridge *bridge=element.bridge;if(!bridge||!bridge.value)return nil;
    char *raw=bridge.value(bridge.context,element.identity);if(!raw)return nil;NSString *value=text(raw);if(bridge.freeValue)bridge.freeValue(raw);
    if(element.role==6||element.role==7)return @([value isEqualToString:@"mixed"]?2:[value isEqualToString:@"1"]?1:0);
    if(element.role==8||element.role==20)return @(value.doubleValue);return value;
}
@implementation CoveAXElement
- (BOOL)invoke:(uint32_t)action value:(NSString*)value{
    NSAssert(NSThread.isMainThread,@"Accessibility actions require the application thread");
    CoveAXElement *keepAlive=self;CoveAXBridge *bridge=keepAlive.bridge;if(!bridge||!(keepAlive.flags&1)||!(keepAlive.flags&512)||!bridge.action)return NO;
    return bridge.action(bridge.context,keepAlive.identity,action,value?value.UTF8String:NULL);
}
#if TARGET_OS_OSX
- (BOOL)accessibilityPerformPress{return (self.actions&1)&&[self invoke:1 value:nil];}
- (BOOL)accessibilityPerformIncrement{return (self.actions&4)&&[self invoke:3 value:nil];}
- (BOOL)accessibilityPerformDecrement{return (self.actions&8)&&[self invoke:4 value:nil];}
- (id)accessibilityValue{return resolved_value(self)?:[super accessibilityValue];}
- (void)setAccessibilityValue:(id)value{if(self.updating){[super setAccessibilityValue:value];return;}if(!(self.actions&2))return;NSString *string=[value isKindOfClass:NSString.class]?value:[value isKindOfClass:NSNumber.class]?[value stringValue]:nil;if(string)[self invoke:2 value:string];}
- (void)setAccessibilityFocused:(BOOL)focused{if(self.updating){[super setAccessibilityFocused:focused];return;}if(focused)[self invoke:5 value:nil];}
- (BOOL)isAccessibilitySelectorAllowed:(SEL)selector{if(selector==@selector(accessibilityPerformPress))return(self.actions&1)!=0;if(selector==@selector(setAccessibilityValue:))return(self.actions&2)!=0;if(selector==@selector(accessibilityPerformIncrement))return(self.actions&4)!=0;if(selector==@selector(accessibilityPerformDecrement))return(self.actions&8)!=0;if(selector==@selector(isAccessibilityExpanded))return(self.flags&4)!=0;if(selector==@selector(setAccessibilityExpanded:))return NO;return[super isAccessibilitySelectorAllowed:selector];}
#else
- (BOOL)accessibilityActivate{return(self.actions&1)&&[self invoke:1 value:nil];}
- (void)accessibilityIncrement{if(self.actions&4)[self invoke:3 value:nil];}
- (void)accessibilityDecrement{if(self.actions&8)[self invoke:4 value:nil];}
- (NSString*)accessibilityValue{id value=resolved_value(self);return[value isKindOfClass:NSString.class]?value:[value respondsToSelector:@selector(stringValue)]?[value stringValue]:[super accessibilityValue];}
- (void)accessibilityElementDidBecomeFocused{if(self.actions&16)[self invoke:5 value:nil];}
#endif
- (void)invalidate{self.bridge=nil;self.presentation=nil;self.childIDs=nil;
#if TARGET_OS_OSX
    [self setAccessibilityChildren:nil];[self setAccessibilityParent:nil];
#else
    self.accessibilityContainer=nil;
#endif
}
@end
void *cove_runtime_ax_create(uintptr_t rawWindow,void *context,CoveAXAction action,CoveAXValue value,CoveAXFreeValue freeValue){NSCAssert(NSThread.isMainThread,@"Accessibility bridges require the main thread");
    CoveAXBridge *bridge=[CoveAXBridge new];bridge.context=context;bridge.action=action;bridge.value=value;bridge.freeValue=freeValue;bridge.elements=[NSMutableDictionary new];
#if TARGET_OS_OSX
    NSWindow *window=(__bridge NSWindow*)(void*)rawWindow;bridge.view=window.contentView;bridge.originalChildren=bridge.view.accessibilityChildren;bridge.originalIsElement=bridge.view.isAccessibilityElement;
#else
    UIWindow *window=(__bridge UIWindow*)(void*)rawWindow;bridge.view=window.rootViewController.view?:window;bridge.originalChildren=bridge.view.accessibilityElements;bridge.originalIsElement=bridge.view.isAccessibilityElement;
#endif
    return(__bridge_retained void*)bridge;
}
void cove_runtime_ax_destroy(void *raw){NSCAssert(NSThread.isMainThread,@"Accessibility bridges require the main thread");if(!raw)return;CoveAXBridge *bridge=CFBridgingRelease(raw);
#if TARGET_OS_OSX
    [bridge.view setAccessibilityChildren:bridge.originalChildren];[bridge.view setAccessibilityElement:bridge.originalIsElement];
#else
    bridge.view.accessibilityElements=bridge.originalChildren;bridge.view.isAccessibilityElement=bridge.originalIsElement;
#endif
    for(CoveAXElement *element in bridge.elements.allValues)[element invalidate];[bridge.elements removeAllObjects];bridge.context=NULL;bridge.action=NULL;bridge.value=NULL;bridge.freeValue=NULL;
}
#if TARGET_OS_OSX
static NSAccessibilityRole native_role(uint32_t role){switch(role){case 1:return NSAccessibilityToolbarRole;case 2:return NSAccessibilityButtonRole;case 3:case 19:return NSAccessibilityStaticTextRole;case 4:return @"AXHeading";case 5:return NSAccessibilityTextFieldRole;case 6:case 7:return NSAccessibilityCheckBoxRole;case 8:return NSAccessibilitySliderRole;case 9:case 10:return NSAccessibilityRadioButtonRole;case 11:return NSAccessibilityListRole;case 12:case 14:case 16:return NSAccessibilityRowRole;case 13:return NSAccessibilityOutlineRole;case 15:return NSAccessibilityTableRole;case 17:return NSAccessibilityCellRole;case 20:return NSAccessibilityProgressIndicatorRole;case 21:return NSAccessibilityImageRole;case 22:return NSAccessibilityLinkRole;case 23:return NSAccessibilityScrollAreaRole;case 24:return NSAccessibilityComboBoxRole;case 25:return NSAccessibilityTabGroupRole;default:return NSAccessibilityGroupRole;}}
#else
static UIAccessibilityTraits native_traits(uint32_t role,uint32_t flags,uint32_t actions){UIAccessibilityTraits result=UIAccessibilityTraitNone;if(role==2||role==6||role==7||role==9||role==10)result|=UIAccessibilityTraitButton;if(role==3||role==19)result|=UIAccessibilityTraitStaticText;if(role==4)result|=UIAccessibilityTraitHeader;if(role==21)result|=UIAccessibilityTraitImage;if(role==22)result|=UIAccessibilityTraitLink;if(actions&12)result|=UIAccessibilityTraitAdjustable;if(flags&2)result|=UIAccessibilityTraitSelected;if(!(flags&1))result|=UIAccessibilityTraitNotEnabled;return result;}
#endif
size_t cove_runtime_ax_update(void *raw,const CoveAXItem *items,size_t count){NSCAssert(NSThread.isMainThread,@"Accessibility updates require the main thread");CoveAXBridge *bridge=(__bridge CoveAXBridge*)raw;if(!bridge||!bridge.view)return 0;
    NSMutableDictionary<NSNumber*,CoveAXElement*> *next=[NSMutableDictionary new];NSMutableDictionary<NSNumber*,NSMutableArray*> *children=[NSMutableDictionary new];NSMutableDictionary<NSNumber*,NSMutableArray*> *childIDs=[NSMutableDictionary new];size_t updated=0;
    for(size_t i=0;i<count;i++){CoveAXItem item=items[i];NSNumber *key=@(item.id);CoveAXElement *element=bridge.elements[key];if(!element){
#if TARGET_OS_OSX
        element=[CoveAXElement new];
#else
        element=[[CoveAXElement alloc]initWithAccessibilityContainer:bridge.view];
#endif
    }element.bridge=bridge;element.identity=item.id;element.flags=item.flags;element.actions=item.actions;element.role=item.role;next[key]=element;NSNumber *parent=@(item.parent);if(!children[parent]){children[parent]=[NSMutableArray new];childIDs[parent]=[NSMutableArray new];}[children[parent] addObject:element];[childIDs[parent] addObject:key];}
    for(size_t i=0;i<count;i++){CoveAXItem item=items[i];CoveAXElement *element=next[@(item.id)];id parent=item.parent?next[@(item.parent)]:bridge.view;
        NSString *identifier=text(item.identifier);if(!identifier.length)identifier=[NSString stringWithFormat:@"e%llu",(unsigned long long)item.id];
        CGRect local=CGRectMake(item.x,item.y,item.width,item.height),screen;
#if TARGET_OS_OSX
        NSView *view=bridge.view;if(!view.isFlipped)local.origin.y=view.bounds.size.height-item.y-item.height;screen=[view.window convertRectToScreen:[view convertRect:local toView:nil]];
#else
        screen=UIAccessibilityConvertFrameToScreenCoordinates(local,bridge.view);
#endif
        NSDictionary *presentation=@{@"role":@(item.role),@"flags":@(item.flags),@"actions":@(item.actions),@"label":text(item.label),@"value":text(item.value),@"help":text(item.help),@"identifier":identifier,@"revision":@(item.revision),@"frame":[NSValue valueWithBytes:&screen objCType:@encode(CGRect)],@"parent":@(item.parent)};
        if(![element.presentation isEqual:presentation]){BOOL valueChanged=element.presentation&&(![element.presentation[@"value"]isEqual:presentation[@"value"]]||![element.presentation[@"revision"]isEqual:presentation[@"revision"]]);element.presentation=presentation;element.updating=YES;
#if TARGET_OS_OSX
            [element setAccessibilityElement:YES];[element setAccessibilityParent:parent];[element setAccessibilityRole:native_role(item.role)];[element setAccessibilitySubrole:(item.flags&128)?NSAccessibilitySecureTextFieldSubrole:nil];[element setAccessibilityLabel:text(item.label)];[element setAccessibilityHelp:text(item.help)];[element setAccessibilityIdentifier:identifier];[element setAccessibilityFrame:screen];[element setAccessibilityEnabled:(item.flags&1)!=0];[element setAccessibilitySelected:(item.flags&2)!=0];[element setAccessibilityFocused:(item.flags&256)!=0];[element setAccessibilityExpanded:(item.flags&8)!=0];
            id value=(item.role==6||item.role==7)?@([text(item.value)isEqualToString:@"mixed"]?2:[text(item.value)isEqualToString:@"1"]?1:0):(item.role==8||item.role==20)?@(text(item.value).doubleValue):text(item.value);[element setAccessibilityValue:value];if(valueChanged)NSAccessibilityPostNotification(element,NSAccessibilityValueChangedNotification);
#else
            element.isAccessibilityElement=YES;element.accessibilityContainer=parent;element.accessibilityLabel=text(item.label);element.accessibilityHint=text(item.help);element.accessibilityIdentifier=identifier;element.accessibilityFrame=screen;element.accessibilityTraits=native_traits(item.role,item.flags,item.actions);if(valueChanged)UIAccessibilityPostNotification(UIAccessibilityLayoutChangedNotification,element);
#endif
            element.updating=NO;updated++;}
        NSArray *ids=childIDs[@(item.id)]?:@[];if(![element.childIDs isEqual:ids]){element.childIDs=ids;
#if TARGET_OS_OSX
            [element setAccessibilityChildren:children[@(item.id)]?:@[]];
#endif
        }
    }
    NSArray *rootIDs=childIDs[@0]?:@[];if(![bridge.rootIDs isEqual:rootIDs]){bridge.rootIDs=rootIDs;
#if TARGET_OS_OSX
        [bridge.view setAccessibilityElement:NO];[bridge.view setAccessibilityChildren:children[@0]?:@[]];NSAccessibilityPostNotification(bridge.view,NSAccessibilityLayoutChangedNotification);
#else
        bridge.view.isAccessibilityElement=NO;bridge.view.accessibilityElements=children[@0]?:@[];UIAccessibilityPostNotification(UIAccessibilityLayoutChangedNotification,bridge.view);
#endif
    }
    for(NSNumber *key in bridge.elements)if(!next[key])[bridge.elements[key] invalidate];bridge.elements=next;return updated;
}
