#include "native.h"
#include <SDL3/SDL.h>
#if defined(__APPLE__)
#include <SDL3/SDL_metal.h>
#endif
#include <math.h>
#include <stdlib.h>
#include <string.h>
struct CoveShellWindow { SDL_Window *window; void *metal_view,*layer,*apple; SDL_Cursor *cursors[12]; CoveShellChrome chrome; CoveShellRect *drag,*non_drag; bool has_chrome; };
static SDL_AtomicInt wake_pending;
static SDL_AtomicInt wake_lost;
static SDL_AtomicInt initialized;
static uint32_t wake_type;
bool cove_shell_is_main_thread(void) { return SDL_IsMainThread(); }
#if defined(COVE_ASSERT_MAIN_THREAD)
void cove_shell_assert_main_thread(const char *function) {
    if (SDL_IsMainThread()) return;
    SDL_LogCritical(SDL_LOG_CATEGORY_APPLICATION,"%s called off the application main thread",function);
    abort();
}
#endif
static uint16_t modifiers(SDL_Keymod value) {
    return ((value & SDL_KMOD_LSHIFT)?1:0) | ((value & SDL_KMOD_RSHIFT)?2:0) | ((value & SDL_KMOD_LCTRL)?4:0) | ((value & SDL_KMOD_RCTRL)?8:0) | ((value & SDL_KMOD_LALT)?16:0) | ((value & SDL_KMOD_RALT)?32:0) | ((value & SDL_KMOD_LGUI)?64:0) | ((value & SDL_KMOD_RGUI)?128:0);
}
bool cove_shell_init(void) {
    if (SDL_GetAtomicInt(&initialized)) return true;
    if (!SDL_IsMainThread()) return SDL_SetError("Cove shell must be initialized on the application main thread");
#if defined(_WIN32)
    SDL_SetHint(SDL_HINT_WINDOWS_DPI_AWARENESS,"permonitorv2");
#endif
#if defined(__APPLE__)
    SDL_SetHint("SDL_MAC_SCROLL_MOMENTUM","1");
#endif
    SDL_SetHint(SDL_HINT_TOUCH_MOUSE_EVENTS,"0");
    if (!SDL_Init(SDL_INIT_VIDEO | SDL_INIT_GAMEPAD)) return false;
    wake_type = SDL_RegisterEvents(1); if (wake_type == (uint32_t)-1) { SDL_Quit(); return false; }
    SDL_SetAtomicInt(&initialized,1); return true;
}
void cove_shell_quit(void) { COVE_ASSERT_MAIN(); if (SDL_GetAtomicInt(&initialized)) { SDL_Quit(); SDL_SetAtomicInt(&initialized,0); SDL_SetAtomicInt(&wake_pending,0); SDL_SetAtomicInt(&wake_lost,0); } }
const char *cove_shell_error(void) { return SDL_GetError(); }
CoveShellWindow *cove_shell_create(const char *title,int width,int height,bool inset,bool dock) { COVE_ASSERT_MAIN();
    if (!cove_shell_init()) return NULL;
#if defined(__APPLE__)
    cove_shell_apple_prepare(dock);
#endif
    SDL_WindowFlags flags = SDL_WINDOW_RESIZABLE | SDL_WINDOW_HIGH_PIXEL_DENSITY;
#if defined(__APPLE__)
    flags |= SDL_WINDOW_METAL;
#else
    if (inset) flags |= SDL_WINDOW_BORDERLESS;
#endif
#if defined(_WIN32)
    float scale = SDL_GetDisplayContentScale(SDL_GetPrimaryDisplay()); if (!isfinite(scale) || scale < 1) scale = 1;
    width = (int)roundf(width * scale); height = (int)roundf(height * scale);
#endif
    CoveShellWindow *handle = calloc(1,sizeof(*handle)); if (!handle) return NULL;
    handle->window = SDL_CreateWindow(title,width,height,flags);
    if (!handle->window) { free(handle); return NULL; }
#if defined(__APPLE__)
    handle->metal_view = SDL_Metal_CreateView(handle->window);
    if (!handle->metal_view) { SDL_DestroyWindow(handle->window); free(handle); return NULL; }
    handle->layer = SDL_Metal_GetLayer(handle->metal_view);
    SDL_PropertiesID props = SDL_GetWindowProperties(handle->window);
    void *window = SDL_GetPointerProperty(props,"SDL.window.cocoa.window",NULL);
    if (!window) window = SDL_GetPointerProperty(props,"SDL.window.uikit.window",NULL);
    handle->apple = cove_shell_apple_attach(window,handle->layer,inset,dock);
#endif
    return handle;
}
void cove_shell_destroy(CoveShellWindow *handle) { COVE_ASSERT_MAIN();
    if (!handle) return;
    SDL_SetWindowHitTest(handle->window,NULL,NULL);
#if defined(__APPLE__)
    cove_shell_apple_detach(handle->apple); if (handle->metal_view) SDL_Metal_DestroyView(handle->metal_view);
#endif
    SDL_DestroyWindow(handle->window); for(int i=0;i<12;i++) if(handle->cursors[i]) SDL_DestroyCursor(handle->cursors[i]);
    free(handle->drag);free(handle->non_drag);free(handle);
}
bool cove_shell_state(CoveShellWindow *handle,CoveShellWindowState *out) { COVE_ASSERT_MAIN();
    if (!handle || !out) return false; memset(out,0,sizeof(*out)); SDL_Window *window = handle->window;
    out->id = SDL_GetWindowID(window); SDL_GetWindowSize(window,&out->width,&out->height); SDL_GetWindowSizeInPixels(window,&out->pixel_width,&out->pixel_height); SDL_GetWindowPosition(window,&out->x,&out->y);
    out->scale = SDL_GetWindowDisplayScale(window); if (!isfinite(out->scale) || out->scale < 1) out->scale = 1;
    SDL_WindowFlags flags = SDL_GetWindowFlags(window);
    out->focused = (flags & SDL_WINDOW_INPUT_FOCUS)!=0; out->visible=(flags & SDL_WINDOW_HIDDEN)==0; out->minimized=(flags & SDL_WINDOW_MINIMIZED)!=0; out->occluded=(flags & SDL_WINDOW_OCCLUDED)!=0; out->maximized=(flags & (SDL_WINDOW_MAXIMIZED | SDL_WINDOW_FULLSCREEN))!=0;
    SDL_Rect safe; if(SDL_GetWindowSafeArea(window,&safe)) { out->top=(float)safe.y;out->leading=(float)safe.x;out->bottom=(float)SDL_max(0,out->height-safe.y-safe.h);out->trailing=(float)SDL_max(0,out->width-safe.x-safe.w); }
    const SDL_DisplayMode *mode=SDL_GetCurrentDisplayMode(SDL_GetDisplayForWindow(window)); if(mode) out->refresh_rate=mode->refresh_rate;
    SDL_PropertiesID props=SDL_GetWindowProperties(window);
#if defined(__APPLE__)
    out->surface_kind=1;out->surface=(uintptr_t)handle->layer;cove_shell_apple_update(handle->apple,out);
#elif defined(_WIN32)
    out->surface_kind=2;out->surface=(uintptr_t)SDL_GetPointerProperty(props,"SDL.window.win32.hwnd",NULL);out->instance=(uintptr_t)SDL_GetPointerProperty(props,"SDL.window.win32.instance",NULL);out->native_window=out->surface;
#elif defined(__ANDROID__)
    out->surface_kind=5;out->surface=(uintptr_t)SDL_GetPointerProperty(props,"SDL.window.android.window",NULL);
#else
    out->display=(uintptr_t)SDL_GetPointerProperty(props,"SDL.window.wayland.display",NULL);out->surface=(uintptr_t)SDL_GetPointerProperty(props,"SDL.window.wayland.surface",NULL);
    if(out->display && out->surface) out->surface_kind=4;
    else { out->display=(uintptr_t)SDL_GetPointerProperty(props,"SDL.window.x11.display",NULL);out->native_number=(uint64_t)SDL_GetNumberProperty(props,"SDL.window.x11.window",0);if(out->display && out->native_number) out->surface_kind=3; }
#endif
    return true;
}
static float coordinate_scale(uint32_t id) {
#if defined(_WIN32)
    SDL_Window *window=SDL_GetWindowFromID(id);float scale=window?SDL_GetWindowDisplayScale(window):1;return isfinite(scale)&&scale>0?scale:1;
#else
    (void)id;return 1;
#endif
}
bool cove_shell_poll(CoveShellEvent *out) { COVE_ASSERT_MAIN();
    SDL_Event e;
    while(SDL_GetAtomicInt(&initialized) && SDL_PollEvent(&e)) {
        memset(out,0,sizeof(*out)); out->window=e.window.windowID;out->timestamp=(double)e.common.timestamp/1e9;
#if defined(__APPLE__)
        if(e.type >= SDL_EVENT_USER && e.type != wake_type) { out->identity=(uintptr_t)e.user.data1;out->kind=e.type; if(cove_shell_apple_event(out)) return true; }
#endif
        if(e.type==wake_type) { SDL_SetAtomicInt(&wake_pending,0);continue; }
        switch(e.type) {
            case SDL_EVENT_QUIT:out->kind=1;break;
            case SDL_EVENT_WINDOW_CLOSE_REQUESTED:out->kind=2;break;
            case SDL_EVENT_WINDOW_DESTROYED:out->kind=3;break;
            case SDL_EVENT_WINDOW_RESIZED:out->kind=4;out->a=e.window.data1;out->b=e.window.data2;break;
            case SDL_EVENT_WINDOW_PIXEL_SIZE_CHANGED:case SDL_EVENT_WINDOW_METAL_VIEW_RESIZED:out->kind=5;out->a=e.window.data1;out->b=e.window.data2;break;
            case SDL_EVENT_WINDOW_FOCUS_GAINED:out->kind=6;break;case SDL_EVENT_WINDOW_FOCUS_LOST:out->kind=7;break;
            case SDL_EVENT_WINDOW_MINIMIZED:out->kind=8;break;case SDL_EVENT_WINDOW_RESTORED:out->kind=9;break;
            case SDL_EVENT_WINDOW_OCCLUDED:out->kind=10;break;case SDL_EVENT_WINDOW_EXPOSED:out->kind=11;break;
            case SDL_EVENT_KEY_DOWN:case SDL_EVENT_KEY_UP:
                out->kind=e.type==SDL_EVENT_KEY_DOWN?12:13;out->window=e.key.windowID;out->a=e.key.scancode;out->b=(int32_t)e.key.key;out->modifiers=modifiers(e.key.mod);out->flags=e.key.repeat?1:0;
#if defined(_WIN32)
                if(out->kind==12 && out->a==44 && (out->modifiers&48)) { SDL_ShowWindowSystemMenu(SDL_GetWindowFromID(out->window),0,0);continue; }
#endif
                break;
            case SDL_EVENT_TEXT_INPUT:out->kind=14;out->window=e.text.windowID;out->text=SDL_strdup(e.text.text?e.text.text:"");break;
            case SDL_EVENT_TEXT_EDITING:out->kind=15;out->window=e.edit.windowID;out->text=SDL_strdup(e.edit.text?e.edit.text:"");out->a=e.edit.start;out->b=e.edit.length;break;
            case SDL_EVENT_MOUSE_MOTION:out->kind=16;out->window=e.motion.windowID;out->x=e.motion.x;out->y=e.motion.y;out->dx=e.motion.xrel;out->dy=e.motion.yrel;break;
            case SDL_EVENT_MOUSE_BUTTON_DOWN:case SDL_EVENT_MOUSE_BUTTON_UP:out->kind=e.type==SDL_EVENT_MOUSE_BUTTON_DOWN?17:18;out->window=e.button.windowID;out->x=e.button.x;out->y=e.button.y;out->a=e.button.button;out->b=e.button.clicks;out->modifiers=modifiers(SDL_GetModState());break;
            case SDL_EVENT_MOUSE_WHEEL:out->kind=19;out->window=e.wheel.windowID;out->dx=-e.wheel.x;out->dy=e.wheel.y;out->x=e.wheel.mouse_x;out->y=e.wheel.mouse_y;if(out->x==0&&out->y==0)SDL_GetMouseState(&out->x,&out->y);break;
            case SDL_EVENT_FINGER_DOWN:case SDL_EVENT_FINGER_UP:case SDL_EVENT_FINGER_MOTION: {
                out->kind=e.type==SDL_EVENT_FINGER_DOWN?17:e.type==SDL_EVENT_FINGER_UP?18:16;out->window=e.tfinger.windowID;out->flags=2;out->a=1;out->b=1;out->identity=e.tfinger.fingerID;
                SDL_Window *window=SDL_GetWindowFromID(out->window);int w=1,h=1;SDL_GetWindowSizeInPixels(window,&w,&h);float scale=SDL_GetWindowDisplayScale(window);if(!isfinite(scale)||scale<1)scale=1;
                out->x=e.tfinger.x*w/scale;out->y=e.tfinger.y*h/scale;out->dx=e.tfinger.dx*w/scale;out->dy=e.tfinger.dy*h/scale;break; }
            case SDL_EVENT_GAMEPAD_BUTTON_DOWN:case SDL_EVENT_GAMEPAD_BUTTON_UP:out->kind=e.type==SDL_EVENT_GAMEPAD_BUTTON_DOWN?20:21;out->identity=e.gbutton.which;out->a=e.gbutton.button;break;
            case SDL_EVENT_GAMEPAD_AXIS_MOTION:out->kind=22;out->identity=e.gaxis.which;out->a=e.gaxis.axis;out->x=SDL_clamp(e.gaxis.value/32767.0f,e.gaxis.axis>=SDL_GAMEPAD_AXIS_LEFT_TRIGGER?0:-1,1);break;
            case SDL_EVENT_GAMEPAD_ADDED:out->kind=23;out->identity=e.gdevice.which;break;case SDL_EVENT_GAMEPAD_REMOVED:out->kind=24;out->identity=e.gdevice.which;break;
            default:continue;
        }
#if defined(__APPLE__)
        if(out->kind==19 && cove_shell_apple_wheel_owned(out->window))continue;
        if(out->kind==12 && cove_shell_apple_shortcut(out->window,out->a,out->modifiers))continue;
#endif
        if(out->kind>=16&&out->kind<=19&&out->flags!=2) { float scale=coordinate_scale(out->window);out->x/=scale;out->y/=scale;if(out->kind==16){out->dx/=scale;out->dy/=scale;} }
        return true;
    }
#if defined(__APPLE__)
    cove_shell_apple_flush();
#endif
    return false;
}
void cove_shell_event_free(CoveShellEvent *event) { SDL_free(event->text);event->text=NULL; }
bool cove_shell_wait(uint64_t timeout) { COVE_ASSERT_MAIN(); if(SDL_CompareAndSwapAtomicInt(&wake_lost,1,0))return true; uint64_t deadline=SDL_GetTicksNS()+SDL_min(timeout,86400000000000ULL);while(SDL_GetAtomicInt(&initialized)){uint64_t now=SDL_GetTicksNS();if(now>=deadline)return false;uint64_t remaining=deadline-now,ms=remaining/1000000;if(ms){if(SDL_WaitEventTimeout(NULL,(int)SDL_min(ms,2147483647)))return true;}else{SDL_DelayNS(remaining);return false;}}return false; }
void cove_shell_wake(void) { if(SDL_GetAtomicInt(&initialized) && SDL_CompareAndSwapAtomicInt(&wake_pending,0,1)){SDL_Event e;SDL_zero(e);e.type=wake_type;if(!SDL_PushEvent(&e)){SDL_SetAtomicInt(&wake_lost,1);SDL_SetAtomicInt(&wake_pending,0);}} }
void cove_shell_command(CoveShellWindow *h,int command,int a,int b) { COVE_ASSERT_MAIN();
    if(!h)return;
#if defined(__APPLE__)
    if(cove_shell_apple_command(h->apple,command))return;
#endif
    switch(command){case 1:SDL_SetWindowPosition(h->window,a,b);break;case 2:SDL_SetWindowSize(h->window,a,b);break;case 3:SDL_RaiseWindow(h->window);break;case 4:SDL_MinimizeWindow(h->window);break;case 5:SDL_MaximizeWindow(h->window);break;case 6:SDL_RestoreWindow(h->window);break;case 7:SDL_HideWindow(h->window);break;case 8:SDL_ShowWindow(h->window);break;case 9:SDL_ShowWindowSystemMenu(h->window,a,b);break;}
}
static bool contains(CoveShellRect r,float x,float y){return x>=r.x&&y>=r.y&&x<r.x+r.width&&y<r.y+r.height;}
static SDL_HitTestResult SDLCALL hit_test(SDL_Window *window,const SDL_Point *point,void *data){
    CoveShellWindow *h=data;CoveShellChrome *c=&h->chrome;float scale=coordinate_scale(SDL_GetWindowID(window));float x=point->x/scale,y=point->y/scale;int rw=0,rh=0;SDL_GetWindowSize(window,&rw,&rh);float w=rw/scale,hh=rh/scale,b=(SDL_GetWindowFlags(window)&SDL_WINDOW_MAXIMIZED)?0:c->border;
    bool l=x<b,r=x>=w-b,t=y<b,bt=y>=hh-b;
    if(b>0){if(t&&l)return SDL_HITTEST_RESIZE_TOPLEFT;if(t&&r)return SDL_HITTEST_RESIZE_TOPRIGHT;if(bt&&l)return SDL_HITTEST_RESIZE_BOTTOMLEFT;if(bt&&r)return SDL_HITTEST_RESIZE_BOTTOMRIGHT;if(t)return SDL_HITTEST_RESIZE_TOP;if(bt)return SDL_HITTEST_RESIZE_BOTTOM;if(l)return SDL_HITTEST_RESIZE_LEFT;if(r)return SDL_HITTEST_RESIZE_RIGHT;}
    for(size_t i=0;i<c->non_drag_count;i++)if(contains(c->non_drag[i],x,y))return SDL_HITTEST_NORMAL;
    if(c->explicit_drag){for(size_t i=0;i<c->drag_count;i++)if(contains(c->drag[i],x,y))return SDL_HITTEST_DRAGGABLE;return SDL_HITTEST_NORMAL;}
    return y>=0&&y<c->title_height&&x>=c->leading&&x<w-c->trailing?SDL_HITTEST_DRAGGABLE:SDL_HITTEST_NORMAL;
}
void cove_shell_chrome(CoveShellWindow *h,const CoveShellChrome *chrome){ COVE_ASSERT_MAIN();
    if(!h)return;SDL_SetWindowHitTest(h->window,NULL,NULL);free(h->drag);free(h->non_drag);h->drag=h->non_drag=NULL;h->has_chrome=false;
    if(!chrome)return;h->chrome=*chrome;
    if(chrome->drag_count){h->drag=malloc(chrome->drag_count*sizeof(*h->drag));if(!h->drag)return;memcpy(h->drag,chrome->drag,chrome->drag_count*sizeof(*h->drag));}
    if(chrome->non_drag_count){h->non_drag=malloc(chrome->non_drag_count*sizeof(*h->non_drag));if(!h->non_drag){free(h->drag);h->drag=NULL;return;}memcpy(h->non_drag,chrome->non_drag,chrome->non_drag_count*sizeof(*h->non_drag));}
    h->chrome.drag=h->drag;h->chrome.non_drag=h->non_drag;h->has_chrome=true;SDL_SetWindowHitTest(h->window,hit_test,h);
#if defined(__APPLE__)
    cove_shell_apple_chrome(h->apple,&h->chrome);
#endif
}
void cove_shell_text_area(CoveShellWindow *h,const float *area){ COVE_ASSERT_MAIN();if(!h)return;if(!area){SDL_SetTextInputArea(h->window,NULL,0);SDL_StopTextInput(h->window);return;}SDL_Rect r={(int)fmaxf(0,floorf(area[0])),(int)fmaxf(0,floorf(area[1])),(int)fmaxf(1,ceilf(area[2])),(int)fmaxf(1,ceilf(area[3]))};if(!SDL_TextInputActive(h->window))SDL_StartTextInput(h->window);SDL_SetTextInputArea(h->window,&r,(int)fmaxf(0,roundf(area[4])));}
void cove_shell_cursor(CoveShellWindow *h,int cursor){ COVE_ASSERT_MAIN();static const SDL_SystemCursor ids[]={SDL_SYSTEM_CURSOR_DEFAULT,SDL_SYSTEM_CURSOR_POINTER,SDL_SYSTEM_CURSOR_TEXT,SDL_SYSTEM_CURSOR_CROSSHAIR,SDL_SYSTEM_CURSOR_WAIT,SDL_SYSTEM_CURSOR_PROGRESS,SDL_SYSTEM_CURSOR_NOT_ALLOWED,SDL_SYSTEM_CURSOR_MOVE,SDL_SYSTEM_CURSOR_EW_RESIZE,SDL_SYSTEM_CURSOR_NS_RESIZE,SDL_SYSTEM_CURSOR_NWSE_RESIZE,SDL_SYSTEM_CURSOR_NESW_RESIZE};if(!h||cursor<0||cursor>=12)return;if(!h->cursors[cursor])h->cursors[cursor]=SDL_CreateSystemCursor(ids[cursor]);if(h->cursors[cursor])SDL_SetCursor(h->cursors[cursor]);}
bool cove_shell_global_pointer(float *x,float *y){ COVE_ASSERT_MAIN();if(!SDL_GetAtomicInt(&initialized))return false;SDL_GetGlobalMouseState(x,y);return true;}
char *cove_shell_clipboard_read(void){ COVE_ASSERT_MAIN();return SDL_GetClipboardText();}bool cove_shell_clipboard_write(const char *text){ COVE_ASSERT_MAIN();return SDL_SetClipboardText(text);}void cove_shell_free(void *p){SDL_free(p);}
struct DialogBox{CoveShellDialogCallback callback;void *context;SDL_DialogFileFilter *filters;char *path;size_t count;};
static void SDLCALL dialog_done(void *raw,const char *const *files,int filter){(void)filter;struct DialogBox *box=raw;box->callback(box->context,files);for(size_t i=0;i<box->count;i++){SDL_free((void*)box->filters[i].name);SDL_free((void*)box->filters[i].pattern);}SDL_free(box->filters);SDL_free(box->path);SDL_free(box);}
void cove_shell_dialog(CoveShellWindow *window,bool folder,const char *path,const char *const *names,const char *const *patterns,size_t count,bool multiple,CoveShellDialogCallback callback,void *context){ COVE_ASSERT_MAIN();
    struct DialogBox *box=SDL_calloc(1,sizeof(*box));if(!box){callback(context,NULL);return;}box->callback=callback;box->context=context;box->path=path?SDL_strdup(path):NULL;box->count=count;
    if(count){box->filters=SDL_calloc(count,sizeof(*box->filters));if(!box->filters){box->count=0;dialog_done(box,NULL,0);return;}for(size_t i=0;i<count;i++){box->filters[i].name=SDL_strdup(names[i]);box->filters[i].pattern=SDL_strdup(patterns[i]);}}
    if(folder)SDL_ShowOpenFolderDialog(dialog_done,box,window?window->window:NULL,box->path,false);else SDL_ShowOpenFileDialog(dialog_done,box,window?window->window:NULL,box->filters,(int)count,box->path,multiple);
}
bool cove_shell_refresh(CoveShellWindow *h,bool enabled){ COVE_ASSERT_MAIN();
#if defined(__APPLE__)
    return h&&cove_shell_apple_refresh(h->apple,enabled);
#else
    (void)h;(void)enabled;return false;
#endif
}
bool cove_shell_refresh_take(CoveShellWindow *h,double *timestamp,double *target){ COVE_ASSERT_MAIN();
#if defined(__APPLE__)
    return h&&cove_shell_apple_refresh_take(h->apple,timestamp,target);
#else
    (void)h;(void)timestamp;(void)target;return false;
#endif
}
static CoveShellLogCallback log_callback;static void SDLCALL log_output(void *context,int category,SDL_LogPriority priority,const char *message){(void)context;if(log_callback)log_callback(category,(int)priority,message);}
void cove_shell_log_install(CoveShellLogCallback callback){log_callback=callback;SDL_SetLogOutputFunction(log_output,NULL);}

double cove_shell_monotonic_seconds(void){return (double)SDL_GetPerformanceCounter()/(double)SDL_GetPerformanceFrequency();}

/* Screen-colour sampling lives in Rust (crate::color_sampler), with one
   asynchronous backend per window system. No C entry point blocks here. */
