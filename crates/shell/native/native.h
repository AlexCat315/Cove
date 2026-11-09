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
/* status: 0 the user chose the listed paths, 1 the user cancelled, 2 the panel
   could not run, 3 the platform exported the bytes itself and does not disclose
   a destination (the list is then empty). */
typedef void (*CoveShellDialogCallback)(void *, const char *const *, int status);
/* mode: 0 open files, 1 open a folder, 2 save a file. */
void cove_shell_dialog(CoveShellWindow *,int mode,const char *path,const char *const *names,const char *const *patterns,size_t count,bool multiple,CoveShellDialogCallback callback,void *context);
/* iOS export: the OS copies `bytes` to the picked destination and reports
   status 3. Elsewhere this reports status 2 (no export picker). */
void cove_shell_save_dialog(CoveShellWindow *,const char *name,const uint8_t *bytes,size_t size,const char *const *patterns,size_t count,CoveShellDialogCallback callback,void *context);
bool cove_shell_refresh(CoveShellWindow *,bool enabled);
bool cove_shell_refresh_take(CoveShellWindow *,double *timestamp,double *target);
typedef void (*CoveShellLogCallback)(int category,int priority,const char *message);
void cove_shell_log_install(CoveShellLogCallback callback);
#if defined(__APPLE__)
/* One menu entry. `parent` indexes this same array, SIZE_MAX for a root.
   kind: 0 regular, 1 separator, 2 submenu. flags: 1 enabled, 2 checked,
   4 command, 8 control, 16 option, 32 shift, 64 template icon. */
typedef struct { size_t parent; int64_t id; uint32_t kind,flags; const char *title,*key; const uint8_t *png; size_t png_size; } CoveShellMenuItem;
typedef struct {
    const char *app_name;
    const char *services,*hide,*hide_others,*show_all,*quit;
    const char *edit,*undo,*redo,*cut,*copy,*paste,*delete_item,*select_all;
    const char *window,*minimize,*zoom,*bring_all_to_front;
    const CoveShellMenuItem *application_items; size_t application_count;
    const CoveShellMenuItem *menus; size_t menu_count;
} CoveShellMenuBar;
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
/* Installs NSApplication.mainMenu. False where the platform has no menu bar. */
bool cove_shell_menu_bar_install(const CoveShellMenuBar *bar);
/* Drains one chosen authored item id. Thread-safe: AppKit enqueues on the main
   thread, the shell drains on the thread that owns the UI. False when empty. */
bool cove_shell_menu_bar_take_choice(int64_t *chosen);
void cove_shell_menu_bar_clear(void);
/* Presents the iOS document picker for mode 0 (open files) or 1 (open a
   folder). Declines on macOS, where SDL owns a real panel. `names`/`patterns`
   are only valid for this call; the callback owns `context` and runs once. */
bool cove_shell_apple_picker(void *context,int mode,const char *path,const char *const *names,const char *const *patterns,size_t count,bool multiple,CoveShellDialogCallback callback,void *box);
/* Exports `bytes` through the iOS picker, which copies them to the destination
   the user picks. False on macOS. Reports an empty list on success because the
   OS does not disclose where the copy landed. */
bool cove_shell_apple_export(void *context,const char *name,const uint8_t *bytes,size_t size,const char *const *patterns,size_t count,CoveShellDialogCallback callback,void *box);
/* Registers the OS "open with" handlers (macOS AppleEvents, iOS openURL). Safe
   to call once on the platform thread. */
void cove_shell_open_documents_install(void);
/* Drains one opened document/URL into an owned NUL-terminated copy the caller
   frees with cove_shell_free. False when the queue is empty. Thread-safe. */
bool cove_shell_open_documents_take(char **out);
#endif
#endif
