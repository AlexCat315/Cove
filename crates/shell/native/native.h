#ifndef COVE_RUST_SHELL_H
#define COVE_RUST_SHELL_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
typedef struct CoveShellWindow CoveShellWindow;
/* `text` carries the payload of an event that has one (typed text, a dropped
   filename or dropped plain text). `aux` carries a secondary string: for drop
   events it is the drag source application, NULL when unavailable. Both are
   owned copies released by cove_shell_event_free. */
typedef struct { uint32_t kind, window; double timestamp; uint64_t identity; int32_t a,b,c,d; float x,y,dx,dy; uint16_t modifiers,flags; char *text, *aux; } CoveShellEvent;
typedef struct { float x,y,width,height; } CoveShellRect;
typedef struct { float title_height,leading,trailing,border; bool explicit_drag; const CoveShellRect *drag,*non_drag; size_t drag_count,non_drag_count; } CoveShellChrome;
typedef struct { uint32_t id; int32_t width,height,pixel_width,pixel_height,x,y; float scale; bool focused,visible,minimized,occluded,maximized; float top,bottom,leading,trailing; uint32_t surface_kind; uintptr_t surface,display,instance,native_window; uint64_t native_number; double refresh_rate; } CoveShellWindowState;
bool cove_shell_is_main_thread(void);
#if defined(COVE_ASSERT_MAIN_THREAD)
void cove_shell_assert_main_thread(const char *function);
#define COVE_ASSERT_MAIN() cove_shell_assert_main_thread(__func__)
#else
#define COVE_ASSERT_MAIN() ((void)0)
#endif
bool cove_shell_init(void);
void cove_shell_quit(void);
const char *cove_shell_error(void);
CoveShellWindow *cove_shell_create(const char *title,int width,int height,bool inset,bool dock);
void cove_shell_destroy(CoveShellWindow *window);
bool cove_shell_backdrop(CoveShellWindow *window,int kind);
void cove_shell_appearance(CoveShellWindow *window,bool dark);
bool cove_shell_state(CoveShellWindow *window,CoveShellWindowState *out);
bool cove_shell_poll(CoveShellEvent *out);
void cove_shell_event_free(CoveShellEvent *event);
bool cove_shell_wait(uint64_t timeout_ns);
void cove_shell_wake(void);
double cove_shell_monotonic_seconds(void);
void cove_shell_command(CoveShellWindow *window,int command,int a,int b);
bool cove_shell_is_fullscreen(CoveShellWindow *h);
void cove_shell_chrome(CoveShellWindow *window,const CoveShellChrome *chrome);
void cove_shell_text_area(CoveShellWindow *window,const float *area);
/* Explicit on-screen keyboard control and its current height in logical
   points. Touch hosts report the IME inset; desktop hosts return 0. */
void cove_shell_keyboard(CoveShellWindow *window,bool show);
float cove_shell_keyboard_height(void);
/* 0 computer, 1 tablet, 2 convertible PC in slate mode with integrated touch. */
int cove_shell_device_form(void);
void cove_shell_cursor(CoveShellWindow *window,int cursor);
bool cove_shell_global_pointer(float *x,float *y);
char *cove_shell_clipboard_read(void);
bool cove_shell_clipboard_write(const char *text);
void cove_shell_free(void *memory);
typedef void (*CoveShellDialogCallback)(void *, const char *const *);
void cove_shell_dialog(CoveShellWindow *,bool folder,const char *path,const char *const *names,const char *const *patterns,size_t count,bool multiple,CoveShellDialogCallback callback,void *context);
bool cove_shell_refresh(CoveShellWindow *,bool enabled);
bool cove_shell_refresh_take(CoveShellWindow *,double *timestamp,double *target);
typedef void (*CoveShellLogCallback)(int category,int priority,const char *message);
void cove_shell_log_install(CoveShellLogCallback callback);
#if defined(__APPLE__)
void cove_shell_apple_prepare(bool dock);
bool cove_shell_apple_wheel_owned(uint32_t id);
bool cove_shell_apple_shortcut(uint32_t id,int scancode,uint16_t modifiers);
void *cove_shell_apple_attach(void *window,void *layer,bool inset,bool dock);
void cove_shell_apple_detach(void *context);
bool cove_shell_apple_backdrop(void *context,int kind);
void cove_shell_apple_appearance(void *context,bool dark);
void cove_shell_apple_update(void *context,CoveShellWindowState *state);
bool cove_shell_apple_command(void *context,int command,int a);
int cove_shell_apple_fullscreen(void *context);
void cove_shell_apple_chrome(void *context,const CoveShellChrome *chrome);
bool cove_shell_apple_refresh(void *context,bool active);
bool cove_shell_apple_refresh_take(void *context,double *timestamp,double *target);
bool cove_shell_apple_event(CoveShellEvent *event);
float cove_shell_apple_keyboard_height(void);
void cove_shell_apple_flush(void);
#endif
#endif
