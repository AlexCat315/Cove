#include "native.h"
#include <SDL3/SDL.h>
#if defined(__APPLE__)
#include <TargetConditionals.h>
#include <SDL3/SDL_metal.h>
#endif
#if defined(_WIN32)
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#endif
#include <math.h>
#include <stdlib.h>
#include <string.h>
struct CoveShellWindow { SDL_Window *window; void *metal_view,*layer,*apple; SDL_Cursor *cursors[12]; CoveShellChrome chrome; CoveShellRect *drag,*non_drag; bool has_chrome; int shape_kind; float shape_radius; bool has_shape; };
static SDL_AtomicInt wake_pending;
static SDL_AtomicInt wake_lost;
static SDL_AtomicInt initialized;
static uint32_t wake_type;
static uint32_t hotkey_type;
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
    /* Windows: SDL3 declares per-monitor-v2 DPI awareness by default, and
       3.4 no longer exports the hint macro. */
#if defined(__APPLE__)
    SDL_SetHint("SDL_MAC_SCROLL_MOMENTUM","1");
#endif
    SDL_SetHint(SDL_HINT_TOUCH_MOUSE_EVENTS,"0");
    if (!SDL_Init(SDL_INIT_VIDEO | SDL_INIT_GAMEPAD)) return false;
    wake_type = SDL_RegisterEvents(1); if (wake_type == (uint32_t)-1) { SDL_Quit(); return false; }
    hotkey_type = SDL_RegisterEvents(1); if (hotkey_type == (uint32_t)-1) { SDL_Quit(); return false; }
    SDL_SetAtomicInt(&initialized,1); return true;
}
void cove_shell_quit(void) { COVE_ASSERT_MAIN(); if (SDL_GetAtomicInt(&initialized)) { SDL_Quit(); SDL_SetAtomicInt(&initialized,0); SDL_SetAtomicInt(&wake_pending,0); SDL_SetAtomicInt(&wake_lost,0); } }
const char *cove_shell_error(void) { return SDL_GetError(); }
CoveShellWindow *cove_shell_create(const char *title,int width,int height,bool inset,bool borderless,bool dock,bool shaped) { COVE_ASSERT_MAIN();
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
    /* A borderless window has no title bar and no system window buttons, so a
       shaped widget is a pure silhouette instead of a dot with traffic lights
       layered on top of it. macOS needs this flag too: HiddenInset keeps the
       buttons but lets content run underneath them. */
    if (borderless) flags |= SDL_WINDOW_BORDERLESS;
    /* SDL_SetWindowShape requires the window to be created transparent. The
       flag makes fully transparent mask pixels click-through on every video
       backend that implements shape (Windows/X11/macOS). A resident widget
       (dock=false) is a utility window, which SDL maps to a skip-taskbar type
       on X11 and to WS_EX_TOOLWINDOW on Windows. */
    if (shaped) flags |= SDL_WINDOW_TRANSPARENT;
    if (!dock) flags |= SDL_WINDOW_UTILITY;
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
/* Builds an alpha mask for a shaped (transparent) window. `kind` 0 = ellipse,
   1 = rounded rectangle; `radius` is in window units for the rounded form.
   Fully transparent pixels become click-through via SDL's per-backend shape
   implementation. Returns false where the backend has no shape support
   (notably Wayland). The mask is copied by SDL and freed here. */
bool cove_shell_set_window_shape(CoveShellWindow *h,int kind,float radius) { COVE_ASSERT_MAIN();
    if (!h || !h->window) return false;
    int width,height; SDL_GetWindowSize(h->window,&width,&height);
    if (width <= 0 || height <= 0) return false;
    SDL_Surface *mask = SDL_CreateSurface(width,height,SDL_PIXELFORMAT_ARGB32);
    if (!mask) return false;
    if (SDL_LockSurface(mask)) {
        Uint32 *pixels = (Uint32 *)mask->pixels;
        int pitch = mask->pitch / (int)sizeof(Uint32);
        float rx = kind == 0 ? width / 2.0f : radius;
        float ry = kind == 0 ? height / 2.0f : radius;
        if (rx < 0) rx = 0; if (ry < 0) ry = 0;
        float cx = (width - 1) / 2.0f, cy = (height - 1) / 2.0f;
        for (int y = 0; y < height; y++) {
            for (int x = 0; x < width; x++) {
                bool inside;
                if (kind == 0) {
                    float dx = (x - cx) / (rx > 0 ? rx : 1.0f);
                    float dy = (y - cy) / (ry > 0 ? ry : 1.0f);
                    inside = dx * dx + dy * dy <= 1.0f;
                } else {
                    float qx = fabsf(x - cx) - (cx - rx);
                    float qy = fabsf(y - cy) - (cy - ry);
                    float ox = fmaxf(qx, 0.0f), oy = fmaxf(qy, 0.0f);
                    inside = fminf(fmaxf(qx, qy), 0.0f) + sqrtf(ox * ox + oy * oy) <= fmaxf(radius, 0.0f);
                }
                pixels[y * pitch + x] = inside ? 0xFFFFFFFFu : 0x00000000u;
            }
        }
        SDL_UnlockSurface(mask);
    }
    bool ok = SDL_SetWindowShape(h->window, mask);
    SDL_DestroySurface(mask);
    /* Remember the shape so a later resize can rebuild the mask at the new
       window size. The mask is pixel-sized, so SDL_SetWindowSize alone would
       leave a shaped window clipped to its original circle. */
    if (ok) {
        h->shape_kind = kind;
        h->shape_radius = radius;
        h->has_shape = true;
    }
    return ok;
}

/* ---- Global hotkeys -----------------------------------------------------
   Modifiers: 1 shift, 2 ctrl, 4 alt, 8 super. `key` is a platform-neutral
   virtual key: ASCII letters ('A'-'Z'), digits ('0'-'9'), or an SDL scancode
   for the rest. A registration pushes a user event carrying the id in
   `user.code` when fired, which the poll loop decodes to kind 30. */
static void emit_hotkey(int id) {
    SDL_Event event; SDL_zero(event); event.type = hotkey_type; event.user.code = id;
    SDL_PushEvent(&event);
}

#if defined(_WIN32)
/* Polling-based detection avoids depending on SDL dispatching WM_HOTKEY to a
   message-only window. Each frame we check the registered combos with
   GetAsyncKeyState; edge detection suppresses repeats. */
#define COVE_MAX_HOTKEYS 32
typedef struct { int id; unsigned mods; int vk; bool was_down; } CoveHotkey;
static CoveHotkey hotkeys[COVE_MAX_HOTKEYS];
static int hotkey_count;
bool cove_shell_hotkey_register(int id,unsigned mods,int key) { COVE_ASSERT_MAIN();
    if (!SDL_GetAtomicInt(&initialized) || id <= 0 || hotkey_count >= COVE_MAX_HOTKEYS) return false;
    int vk;
    if (key >= 'A' && key <= 'Z') vk = key;
    else if (key >= '0' && key <= '9') vk = key;
    else if (key >= VK_F1 && key <= VK_F12) vk = key; /* caller passes a Win32 VK */
    else vk = 0;
    if (!vk) return false;
    for (int i = 0; i < hotkey_count; i++) if (hotkeys[i].id == id) { hotkeys[i].mods=mods; hotkeys[i].vk=vk; hotkeys[i].was_down=false; return true; }
    hotkeys[hotkey_count++] = (CoveHotkey){ id, mods, vk, false };
    return true;
}
void cove_shell_hotkey_unregister(int id) { COVE_ASSERT_MAIN();
    for (int i = 0; i < hotkey_count; i++) if (hotkeys[i].id == id) {
        for (int j = i; j + 1 < hotkey_count; j++) hotkeys[j] = hotkeys[j + 1];
        hotkey_count--; return;
    }
}
void cove_shell_hotkey_poll(void) { COVE_ASSERT_MAIN();
    for (int i = 0; i < hotkey_count; i++) {
        CoveHotkey *h = &hotkeys[i];
        bool shift = (GetAsyncKeyState(VK_SHIFT) & 0x8000) != 0;
        bool ctrl = (GetAsyncKeyState(VK_CONTROL) & 0x8000) != 0;
        bool alt = (GetAsyncKeyState(VK_MENU) & 0x8000) != 0;
        bool win = (GetAsyncKeyState(VK_LWIN) & 0x8000) || (GetAsyncKeyState(VK_RWIN) & 0x8000);
        bool mods_ok = ((h->mods & 1) ? shift : !shift) && ((h->mods & 2) ? ctrl : !ctrl)
                    && ((h->mods & 4) ? alt : !alt) && ((h->mods & 8) ? win : !win);
        bool down = mods_ok && (GetAsyncKeyState(h->vk) & 0x8000) != 0;
        if (down && !h->was_down) emit_hotkey(h->id);
        h->was_down = down;
    }
}
#elif defined(__APPLE__)
/* Implemented in apple.m via Carbon RegisterEventHotKey; declared in native.h
   under __APPLE__ and emitted through cove_shell_hotkey_emit. */
bool cove_shell_hotkey_emit(int id) { (void)id; return false; }
#elif defined(__linux__)
/* X11 hotkeys via dlopen'd libX11, no link-time dependency. We poll the key
   map each frame (like the Windows path) using a private Display connection,
   which avoids competing with SDL's event loop for XGrabKey events. Returns
   false on Wayland / when X11 is unavailable. */
#include <dlfcn.h>
typedef struct _XDisplay XDisplay;
typedef unsigned long XID;
static void *x11_lib;
static XDisplay *x11_display;
static int (*x_open_display)(const char *);
static int (*x_close_display)(XDisplay *);
static unsigned long (*x_keysym_to_keycode)(XDisplay *, unsigned long);
static int (*x_query_keymap)(XDisplay *, char *);
#define COVE_MAX_HOTKEYS 32
typedef struct { int id; unsigned mods; int keycode; bool was_down; } CoveHotkey;
static CoveHotkey hotkeys[COVE_MAX_HOTKEYS];
static int hotkey_count;
static unsigned long hotkey_keysym(int key) {
    if (key >= 'A' && key <= 'Z') return key + 0x60; /* XK_a */
    if (key >= 'a' && key <= 'z') return key;
    if (key >= '0' && key <= '9') return (unsigned long)key;
    if (key >= 0x70 && key <= 0x7B) return 0xFFBE + (key - 0x70); /* VK_F1..F12 -> XK_F1 */
    return 0;
}
static bool x11_hotkey_init(void) {
    if (x11_lib) return x11_display != NULL;
    x11_lib = dlopen("libX11.so.6", RTLD_LAZY);
    if (!x11_lib) return false;
    x_open_display = (int (*)(const char *))dlsym(x11_lib, "XOpenDisplay");
    x_close_display = (int (*)(XDisplay *))dlsym(x11_lib, "XCloseDisplay");
    x_keysym_to_keycode = (unsigned long (*)(XDisplay *, unsigned long))dlsym(x11_lib, "XKeysymToKeycode");
    x_query_keymap = (int (*)(XDisplay *, char *))dlsym(x11_lib, "XQueryKeymap");
    if (!x_open_display || !x_query_keymap || !x_keysym_to_keycode) return false;
    x11_display = (XDisplay *)x_open_display(NULL);
    return x11_display != NULL;
}
bool cove_shell_hotkey_register(int id,unsigned mods,int key) { COVE_ASSERT_MAIN();
    if (id <= 0 || hotkey_count >= COVE_MAX_HOTKEYS || !x11_hotkey_init()) return false;
    unsigned long sym = hotkey_keysym(key);
    if (!sym) return false;
    int keycode = (int)x_keysym_to_keycode(x11_display, sym);
    if (!keycode) return false;
    for (int i = 0; i < hotkey_count; i++) if (hotkeys[i].id == id) { hotkeys[i].mods=mods; hotkeys[i].keycode=keycode; hotkeys[i].was_down=false; return true; }
    hotkeys[hotkey_count++] = (CoveHotkey){ id, mods, keycode, false };
    return true;
}
void cove_shell_hotkey_unregister(int id) { COVE_ASSERT_MAIN();
    for (int i = 0; i < hotkey_count; i++) if (hotkeys[i].id == id) {
        for (int j = i; j + 1 < hotkey_count; j++) hotkeys[j] = hotkeys[j + 1];
        hotkey_count--; return;
    }
}
void cove_shell_hotkey_poll(void) { COVE_ASSERT_MAIN();
    if (!x11_display || hotkey_count == 0) return;
    char keymap[32]; if (!x_query_keymap(x11_display, keymap)) return;
    for (int i = 0; i < hotkey_count; i++) {
        CoveHotkey *h = &hotkeys[i];
        bool down = (keymap[h->keycode / 8] & (1 << (h->keycode % 8))) != 0;
        if (down && !h->was_down) emit_hotkey(h->id);
        h->was_down = down;
    }
}
#else
bool cove_shell_hotkey_register(int id,unsigned mods,int key) { (void)id;(void)mods;(void)key;return false; }
void cove_shell_hotkey_unregister(int id) { (void)id; }
#endif

/* Called by platform callbacks (including apple.m and the X11 monitor) to
   deliver a hotkey into the SDL event stream from any thread. */
void cove_shell_hotkey_fire(int id) { emit_hotkey(id); }

/* kind 0 is opaque, 1 the sidebar material. Windows and Linux report false:
   their DXGI and Vulkan swapchains here are created opaque, so a system
   backdrop (Mica, compositor blur) could not show through. */
bool cove_shell_backdrop(CoveShellWindow *handle,int kind) { COVE_ASSERT_MAIN();
#if defined(__APPLE__)
    return handle&&cove_shell_apple_backdrop(handle->apple,kind);
#else
    (void)handle;(void)kind;return false;
#endif
}
/* Native chrome follows the UI theme rather than the system setting, so the
   title bar, traffic lights and materials match what the app paints. Linux
   window decorations belong to the compositor and have no portable switch. */
void cove_shell_appearance(CoveShellWindow *handle,bool dark) { COVE_ASSERT_MAIN();
    if (!handle) return;
#if defined(__APPLE__)
    cove_shell_apple_appearance(handle->apple,dark);
#elif defined(_WIN32)
    typedef long (__stdcall *DwmSetWindowAttributeFn)(void *,unsigned long,const void *,unsigned long);
    static SDL_SharedObject *dwm; static DwmSetWindowAttributeFn set_attribute;
    if (!dwm) { dwm = SDL_LoadObject("dwmapi.dll"); if (dwm) set_attribute = (DwmSetWindowAttributeFn)SDL_LoadFunction(dwm,"DwmSetWindowAttribute"); }
    void *hwnd = SDL_GetPointerProperty(SDL_GetWindowProperties(handle->window),"SDL.window.win32.hwnd",NULL);
    int value = dark ? 1 : 0;
    /* DWMWA_USE_IMMERSIVE_DARK_MODE */
    if (set_attribute && hwnd) set_attribute(hwnd,20,&value,sizeof(value));
#else
    (void)dark;
#endif
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
#if defined(_WIN32) || defined(__linux__)
    cove_shell_hotkey_poll();
#endif
    while(SDL_GetAtomicInt(&initialized) && SDL_PollEvent(&e)) {
        memset(out,0,sizeof(*out)); out->window=e.window.windowID;out->timestamp=(double)e.common.timestamp/1e9;
        if(e.type==hotkey_type) { out->kind=30; out->a=e.user.code; return true; }
#if defined(__APPLE__)
        if(e.type >= SDL_EVENT_USER && e.type != wake_type && e.type != hotkey_type) { out->identity=(uintptr_t)e.user.data1;out->kind=e.type; if(cove_shell_apple_event(out)) return true; }
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
            case SDL_EVENT_DROP_BEGIN:case SDL_EVENT_DROP_FILE:case SDL_EVENT_DROP_TEXT:case SDL_EVENT_DROP_POSITION:case SDL_EVENT_DROP_COMPLETE: {
                out->kind=e.type==SDL_EVENT_DROP_BEGIN?25:e.type==SDL_EVENT_DROP_FILE?26:e.type==SDL_EVENT_DROP_TEXT?27:e.type==SDL_EVENT_DROP_POSITION?28:29;
                out->window=e.drop.windowID;out->x=e.drop.x;out->y=e.drop.y;
                if(e.drop.data)out->text=SDL_strdup(e.drop.data);
                if(e.drop.source)out->aux=SDL_strdup(e.drop.source);
                break; }
            default:continue;
        }
#if defined(__APPLE__)
        if(out->kind==19 && cove_shell_apple_wheel_owned(out->window))continue;
        if(out->kind==12 && cove_shell_apple_shortcut(out->window,out->a,out->modifiers))continue;
#endif
        if(out->kind>=16&&out->kind<=19&&out->flags!=2) { float scale=coordinate_scale(out->window);out->x/=scale;out->y/=scale;if(out->kind==16){out->dx/=scale;out->dy/=scale;} }
        /* Drop positions arrive in the platform's window points; pointer events
           use the same space, so the same scale applies. */
        if(out->kind>=26&&out->kind<=28) { float scale=coordinate_scale(out->window);out->x/=scale;out->y/=scale; }
        return true;
    }
#if defined(__APPLE__)
    cove_shell_apple_flush();
#endif
    return false;
}
void cove_shell_event_free(CoveShellEvent *event) { SDL_free(event->text);event->text=NULL;SDL_free(event->aux);event->aux=NULL; }
bool cove_shell_wait(uint64_t timeout) { COVE_ASSERT_MAIN(); if(SDL_CompareAndSwapAtomicInt(&wake_lost,1,0))return true; uint64_t deadline=SDL_GetTicksNS()+SDL_min(timeout,86400000000000ULL);while(SDL_GetAtomicInt(&initialized)){uint64_t now=SDL_GetTicksNS();if(now>=deadline)return false;uint64_t remaining=deadline-now,ms=remaining/1000000;if(ms){if(SDL_WaitEventTimeout(NULL,(int)SDL_min(ms,2147483647)))return true;}else{SDL_DelayNS(remaining);return false;}}return false; }
void cove_shell_wake(void) { if(SDL_GetAtomicInt(&initialized) && SDL_CompareAndSwapAtomicInt(&wake_pending,0,1)){SDL_Event e;SDL_zero(e);e.type=wake_type;if(!SDL_PushEvent(&e)){SDL_SetAtomicInt(&wake_lost,1);SDL_SetAtomicInt(&wake_pending,0);}} }
void cove_shell_command(CoveShellWindow *h,int command,int a,int b) { COVE_ASSERT_MAIN();
    if(!h)return;
#if defined(__APPLE__)
    if(cove_shell_apple_command(h->apple,command,a))return;
#endif
    switch(command){case 1:SDL_SetWindowPosition(h->window,a,b);break;case 2:SDL_SetWindowSize(h->window,a,b);if(h->has_shape)cove_shell_set_window_shape(h,h->shape_kind,h->shape_radius);break;case 3:SDL_RaiseWindow(h->window);break;case 4:SDL_MinimizeWindow(h->window);break;case 5:SDL_MaximizeWindow(h->window);break;case 6:SDL_RestoreWindow(h->window);break;case 7:SDL_HideWindow(h->window);break;case 8:SDL_ShowWindow(h->window);break;case 9:SDL_ShowWindowSystemMenu(h->window,a,b);break;case 10:SDL_SetWindowFullscreen(h->window,a!=0);break;case 11:SDL_SetWindowAlwaysOnTop(h->window,a!=0);break;case 12:SDL_SetWindowMinimumSize(h->window,a,b);break;}
}
void cove_shell_set_title(CoveShellWindow *h,const char *title){ COVE_ASSERT_MAIN();
    if(!h||!title)return;
    SDL_SetWindowTitle(h->window,title);
}
bool cove_shell_is_fullscreen(CoveShellWindow *h) { COVE_ASSERT_MAIN();
    if(!h)return false;
#if defined(__APPLE__)
    int apple=cove_shell_apple_fullscreen(h->apple);if(apple>=0)return apple!=0;
#endif
    return (SDL_GetWindowFlags(h->window)&SDL_WINDOW_FULLSCREEN)!=0;
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
void cove_shell_text_area(CoveShellWindow *h,const float *area){ COVE_ASSERT_MAIN();if(!h)return;if(!area){SDL_SetTextInputArea(h->window,NULL,0);SDL_StopTextInput(h->window);return;}
    /* Cove lays out in logical points. Windows SDL reports and accepts text
       input rectangles in physical client pixels, so an unconverted area puts
       the IME composition string and candidate window at 1/scale of the caret
       on a 150%/200% display. macOS and Linux report scale 1.0 here. */
    const float scale=coordinate_scale(SDL_GetWindowID(h->window));
    SDL_Rect r={(int)fmaxf(0,floorf(area[0]*scale)),(int)fmaxf(0,floorf(area[1]*scale)),(int)fmaxf(1,ceilf(area[2]*scale)),(int)fmaxf(1,ceilf(area[3]*scale))};if(!SDL_TextInputActive(h->window))SDL_StartTextInput(h->window);SDL_SetTextInputArea(h->window,&r,(int)fmaxf(0,roundf(area[4]*scale)));}
void cove_shell_keyboard(CoveShellWindow *h,bool show){ COVE_ASSERT_MAIN();if(!h)return;if(show){if(!SDL_TextInputActive(h->window))SDL_StartTextInput(h->window);}else SDL_StopTextInput(h->window);}
float cove_shell_keyboard_height(void){
#if defined(__APPLE__)
    return cove_shell_apple_keyboard_height();
#else
    return 0.0f;
#endif
}
int cove_shell_system_theme(void){
    switch (SDL_GetSystemTheme()) {
    case SDL_SYSTEM_THEME_LIGHT: return 1;
    case SDL_SYSTEM_THEME_DARK: return 2;
    default: return 0;
    }
}
int cove_shell_device_form(void){
    if (SDL_IsTablet()) return 1;
#if defined(_WIN32)
    /* SM_CONVERTIBLESLATEMODE is 0 in slate mode; desktops without an
       integrated digitizer may also report 0, so require touch hardware. */
    int digitizer = GetSystemMetrics(SM_DIGITIZER);
    bool touch = (digitizer & NID_INTEGRATED_TOUCH) && (digitizer & NID_READY);
    if (touch && GetSystemMetrics(SM_CONVERTIBLESLATEMODE) == 0) return 2;
#endif
    return 0;
}
void cove_shell_cursor(CoveShellWindow *h,int cursor){ COVE_ASSERT_MAIN();static const SDL_SystemCursor ids[]={SDL_SYSTEM_CURSOR_DEFAULT,SDL_SYSTEM_CURSOR_POINTER,SDL_SYSTEM_CURSOR_TEXT,SDL_SYSTEM_CURSOR_CROSSHAIR,SDL_SYSTEM_CURSOR_WAIT,SDL_SYSTEM_CURSOR_PROGRESS,SDL_SYSTEM_CURSOR_NOT_ALLOWED,SDL_SYSTEM_CURSOR_MOVE,SDL_SYSTEM_CURSOR_EW_RESIZE,SDL_SYSTEM_CURSOR_NS_RESIZE,SDL_SYSTEM_CURSOR_NWSE_RESIZE,SDL_SYSTEM_CURSOR_NESW_RESIZE};if(!h||cursor<0||cursor>=12)return;if(!h->cursors[cursor])h->cursors[cursor]=SDL_CreateSystemCursor(ids[cursor]);if(h->cursors[cursor])SDL_SetCursor(h->cursors[cursor]);}
bool cove_shell_global_pointer(float *x,float *y){ COVE_ASSERT_MAIN();if(!SDL_GetAtomicInt(&initialized))return false;SDL_GetGlobalMouseState(x,y);return true;}

/* Display geometry for placing windows (e.g. a floating widget) on a specific
   monitor. SDL owns enumeration on every platform. `bounds` is in SDL window
   units and `usable` excludes the taskbar/menu bar where SDL reports it. */
int cove_shell_display_count(void) { COVE_ASSERT_MAIN();
    if (!SDL_GetAtomicInt(&initialized)) return 0;
    int count = 0; SDL_DisplayID *ids = SDL_GetDisplays(&count);
    if (!ids) return 0;
    SDL_free(ids); return count;
}
bool cove_shell_display_bounds(int index,bool usable,CoveShellRect *out) { COVE_ASSERT_MAIN();
    if (!out || !SDL_GetAtomicInt(&initialized)) return false;
    int count = 0; SDL_DisplayID *ids = SDL_GetDisplays(&count);
    if (!ids || index < 0 || index >= count) { SDL_free(ids); return false; }
    SDL_DisplayID id = ids[index]; SDL_free(ids);
    SDL_Rect rect;
    bool ok = usable ? SDL_GetDisplayUsableBounds(id, &rect) : SDL_GetDisplayBounds(id, &rect);
    if (!ok) return false;
    out->x=(float)rect.x;out->y=(float)rect.y;out->width=(float)rect.w;out->height=(float)rect.h;
    return true;
}

/* Status-bar tray for platforms without AppKit's NSStatusItem (native.c is not
   compiled on Apple targets). SDL owns the native implementation on Windows and
   X11, so one bridge covers both. The Rust side keeps the menu model and drains
   picks on the UI thread; each entry carries its index as userdata. */
#if !defined(__APPLE__) && !defined(__ANDROID__) && !defined(__IOS__)
static SDL_Tray *tray = NULL;
typedef void (*CoveStatusItemCallback)(size_t index);
static CoveStatusItemCallback tray_callback = NULL;
static void SDLCALL tray_picked(void *raw, SDL_TrayEntry *entry) {
    (void)entry;
    if (!tray_callback) return;
    size_t index = (size_t)(uintptr_t)raw;
    tray_callback(index);
    cove_shell_wake();
}
void cove_shell_tray_destroy(void) {
    if (tray) { SDL_DestroyTray(tray); tray = NULL; }
    tray_callback = NULL;
}
bool cove_shell_tray_install(const char *icon_path,const char *tooltip,
                             const char *const *titles,const char *const *ids,
                             size_t count,CoveStatusItemCallback callback) {
    COVE_ASSERT_MAIN();
    if (!SDL_GetAtomicInt(&initialized) || !callback) return false;
    if (tray) SDL_DestroyTray(tray);
    tray_callback = callback;
    SDL_Surface *icon = NULL;
    if (icon_path && *icon_path) icon = SDL_LoadBMP(icon_path);
    tray = SDL_CreateTray(icon, tooltip && *tooltip ? tooltip : "Cove");
    if (icon) SDL_DestroySurface(icon);
    if (!tray) { tray_callback = NULL; return false; }
    SDL_TrayMenu *menu = SDL_GetTrayMenu(tray);
    if (!menu) { SDL_DestroyTray(tray); tray = NULL; tray_callback = NULL; return false; }
    for (size_t i = 0; i < count; i++) {
        const char *label = titles[i];
        if (!label || !*label) {
            SDL_InsertTrayEntryAt(menu, -1, NULL, SDL_TRAYENTRY_BUTTON);
            continue;
        }
        SDL_TrayEntryFlags flags = SDL_TRAYENTRY_BUTTON;
        (void)ids; /* entry ids are resolved to indices on the Rust side */
        SDL_TrayEntry *entry = SDL_InsertTrayEntryAt(menu, -1, label, flags);
        if (entry) SDL_SetTrayEntryCallback(entry, tray_picked, (void *)(uintptr_t)i);
    }
    SDL_UpdateTrays();
    return true;
}
#endif

char *cove_shell_clipboard_read(void){ COVE_ASSERT_MAIN();return SDL_GetClipboardText();}bool cove_shell_clipboard_write(const char *text){ COVE_ASSERT_MAIN();return SDL_SetClipboardText(text);}void cove_shell_free(void *p){SDL_free(p);}
struct DialogBox{CoveShellDialogCallback callback;void *context;SDL_DialogFileFilter *filters;char *path;size_t count;};
static void dialog_box_free(struct DialogBox *box){for(size_t i=0;i<box->count;i++){SDL_free((void*)box->filters[i].name);SDL_free((void*)box->filters[i].pattern);}SDL_free(box->filters);SDL_free(box->path);SDL_free(box);}
/* SDL's third argument is the chosen filter index, but the macOS backend
   reports -1 for every outcome, a successful pick included, so the file list
   alone decides: NULL means the panel failed, an empty first entry means the
   user cancelled, a path means a choice. */
static void SDLCALL dialog_done(void *raw,const char *const *files,int filter){(void)filter;struct DialogBox *box=raw;int status;if(!files){status=2;}else if(files[0]){status=0;}else{status=1;}box->callback(box->context,files,status);dialog_box_free(box);}
#if defined(__APPLE__)
/* The picker already speaks the cove status convention, so forward it verbatim
   and only borrow the box's freeing. */
static void apple_picker_done(void *raw,const char *const *files,int status){struct DialogBox *box=raw;box->callback(box->context,files,status);dialog_box_free(box);}
#endif
void cove_shell_dialog(CoveShellWindow *window,int mode,const char *path,const char *const *names,const char *const *patterns,size_t count,bool multiple,CoveShellDialogCallback callback,void *context){ COVE_ASSERT_MAIN();
    struct DialogBox *box=SDL_calloc(1,sizeof(*box));if(!box){callback(context,NULL,1);return;}box->callback=callback;box->context=context;box->path=path?SDL_strdup(path):NULL;box->count=count;
    if(count){box->filters=SDL_calloc(count,sizeof(*box->filters));if(!box->filters){box->count=0;dialog_done(box,NULL,-1);return;}for(size_t i=0;i<count;i++){box->filters[i].name=SDL_strdup(names[i]);box->filters[i].pattern=SDL_strdup(patterns[i]);}}
#if defined(__APPLE__)
    /* SDL has no iOS dialog backend, so Cove presents the document picker
       itself; on macOS the hook declines and SDL owns the panel. */
    if(mode!=2&&cove_shell_apple_picker(window?window->apple:NULL,mode,box->path,names,patterns,count,multiple,apple_picker_done,box))return;
#endif
    switch(mode){
        case 1:SDL_ShowOpenFolderDialog(dialog_done,box,window?window->window:NULL,box->path,false);break;
        case 2:SDL_ShowSaveFileDialog(dialog_done,box,window?window->window:NULL,box->filters,(int)count,box->path);break;
        default:SDL_ShowOpenFileDialog(dialog_done,box,window?window->window:NULL,box->filters,(int)count,box->path,multiple);break;
    }
}
void cove_shell_save_dialog(CoveShellWindow *window,const char *name,const uint8_t *bytes,size_t size,const char *const *patterns,size_t count,CoveShellDialogCallback callback,void *context){ COVE_ASSERT_MAIN();
#if defined(__APPLE__) && !TARGET_OS_OSX
    if(cove_shell_apple_export(window?window->apple:NULL,name,bytes,size,patterns,count,callback,context))return;
#else
    (void)window;(void)name;(void)bytes;(void)size;(void)patterns;(void)count;
#endif
    callback(context,NULL,2);
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

struct CoveShellAudioStream { SDL_AudioStream *stream; };
typedef struct { SDL_AtomicInt references; bool success; char error[512]; } CoveAudioInitResult;
static SDL_SpinLock cove_audio_init_lock;
static void cove_audio_init_release(CoveAudioInitResult *result) {
    if (SDL_AddAtomicInt(&result->references,-1) == 1) free(result);
}
static void SDLCALL cove_audio_init_on_main(void *userdata) {
    CoveAudioInitResult *result = userdata;
    SDL_LockSpinlock(&cove_audio_init_lock);
    if (!SDL_IsMainThread()) {
        SDL_strlcpy(result->error,"audio initialization requires the SDL main thread",sizeof(result->error));
        SDL_UnlockSpinlock(&cove_audio_init_lock);
        cove_audio_init_release(result);
        return;
    }
    result->success = SDL_InitSubSystem(SDL_INIT_AUDIO);
    if (!result->success) SDL_strlcpy(result->error,SDL_GetError(),sizeof(result->error));
    SDL_UnlockSpinlock(&cove_audio_init_lock);
    cove_audio_init_release(result);
}
static void SDLCALL cove_audio_quit_on_main(void *userdata) {
    (void)userdata;
    if (!SDL_IsMainThread()) { SDL_LogError(SDL_LOG_CATEGORY_AUDIO,"cannot release audio subsystem without a main event loop"); return; }
    SDL_QuitSubSystem(SDL_INIT_AUDIO);
}
bool cove_shell_audio_init(void) {
    CoveAudioInitResult *result = calloc(1,sizeof(*result));
    if (!result) return SDL_OutOfMemory();
    /* The callback retains its own reference: SDL may report a dispatch
       timeout while the queued callback is still able to run later. */
    SDL_SetAtomicInt(&result->references,2);
    if (!SDL_RunOnMainThread(cove_audio_init_on_main,result,true)) {
        /* On timeout SDL can still run the callback; all other failure paths
           either failed to queue it or canceled it before invocation. */
        if (SDL_strcmp(SDL_GetError(),"Callback timed out") != 0)
            cove_audio_init_release(result);
        cove_audio_init_release(result);
        return false;
    }
    bool success = result->success;
    if (!success) SDL_SetError("%s",result->error);
    cove_audio_init_release(result);
    return success;
}
void cove_shell_audio_quit(void) {
    if (!SDL_RunOnMainThread(cove_audio_quit_on_main,NULL,false))
        SDL_LogError(SDL_LOG_CATEGORY_AUDIO,"cannot dispatch audio shutdown: %s",SDL_GetError());
}
CoveShellAudioStream *cove_shell_audio_open_default(int frequency,int channels) {
    if (frequency <= 0 || channels <= 0) { SDL_SetError("invalid audio stream format"); return NULL; }
    if (!cove_shell_audio_init()) return NULL;
    SDL_AudioSpec spec; spec.format = SDL_AUDIO_F32; spec.channels = channels; spec.freq = frequency;
    SDL_AudioStream *stream = SDL_OpenAudioDeviceStream(SDL_AUDIO_DEVICE_DEFAULT_PLAYBACK,&spec,NULL,NULL);
    if (!stream) {
        char error[512]; SDL_strlcpy(error,SDL_GetError(),sizeof(error));
        cove_shell_audio_quit(); SDL_SetError("%s",error); return NULL;
    }
    CoveShellAudioStream *handle = calloc(1,sizeof(*handle));
    if (!handle) { SDL_DestroyAudioStream(stream); cove_shell_audio_quit(); SDL_OutOfMemory(); return NULL; }
    handle->stream = stream;
    return handle;
}
bool cove_shell_audio_format(CoveShellAudioStream *handle,int *format,int *channels,int *frequency) {
    if (!handle) return false;
    SDL_AudioSpec source,device;
    if (!SDL_GetAudioStreamFormat(handle->stream,&source,&device)) return false;
    if (format) *format = (int)device.format;
    if (channels) *channels = device.channels;
    if (frequency) *frequency = device.freq;
    return true;
}
int cove_shell_audio_queued(CoveShellAudioStream *handle) {
    if (!handle) return 0;
    int queued = SDL_GetAudioStreamQueued(handle->stream);
    return queued < 0 ? 0 : queued;
}
int cove_shell_audio_available(CoveShellAudioStream *handle) {
    if (!handle) return 0;
    int available = SDL_GetAudioStreamAvailable(handle->stream);
    return available < 0 ? 0 : available;
}
bool cove_shell_audio_put(CoveShellAudioStream *handle,const void *bytes,int length) {
    if (!handle || !bytes || length <= 0) return false;
    return SDL_PutAudioStreamData(handle->stream,bytes,length);
}
bool cove_shell_audio_set_gain(CoveShellAudioStream *handle,float gain) { return handle && SDL_SetAudioStreamGain(handle->stream,gain); }
bool cove_shell_audio_set_frequency_ratio(CoveShellAudioStream *handle,float ratio) { return handle && SDL_SetAudioStreamFrequencyRatio(handle->stream,ratio); }
bool cove_shell_audio_flush(CoveShellAudioStream *handle) { return handle && SDL_FlushAudioStream(handle->stream); }
bool cove_shell_audio_pause(CoveShellAudioStream *handle) { return handle && SDL_PauseAudioStreamDevice(handle->stream); }
bool cove_shell_audio_resume(CoveShellAudioStream *handle) { return handle && SDL_ResumeAudioStreamDevice(handle->stream); }
void cove_shell_audio_close(CoveShellAudioStream *handle) {
    if (!handle) return;
    SDL_DestroyAudioStream(handle->stream);
    free(handle);
    cove_shell_audio_quit();
}
