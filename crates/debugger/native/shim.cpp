/* Cove native LLDB bridge: implements the C API in cove_lldb.h over the LLDB SB
 * API. One Session owns an SBDebugger, target, process and listener. A Rust
 * event thread drives cove_lldb_wait_event; every other call is a direct SB
 * operation. Frame and value handles are indices into tables that are cleared
 * whenever the debuggee resumes, so a handle never outlives its stop.
 */
#include "cove_lldb.h"

#include <lldb/API/SBAddress.h>
#include <lldb/API/SBBreakpoint.h>
#include <lldb/API/SBBreakpointLocation.h>
#include <lldb/API/SBBroadcaster.h>
#include <lldb/API/SBDebugger.h>
#include <lldb/API/SBError.h>
#include <lldb/API/SBEvent.h>
#include <lldb/API/SBExpressionOptions.h>
#include <lldb/API/SBFileSpec.h>
#include <lldb/API/SBFrame.h>
#include <lldb/API/SBLineEntry.h>
#include <lldb/API/SBListener.h>
#include <lldb/API/SBProcess.h>
#include <lldb/API/SBStream.h>
#include <lldb/API/SBTarget.h>
#include <lldb/API/SBThread.h>
#include <lldb/API/SBValue.h>
#include <lldb/API/SBValueList.h>

#include <cstdio>
#include <cstring>
#include <deque>
#include <map>
#include <memory>
#include <mutex>
#include <string>
#include <vector>

namespace {

const size_t kMaxOutput = 512;
const uint32_t kWakeEvent = 1;

thread_local std::string g_error;

void set_error(const char *message) { g_error = message ? message : "unknown error"; }
void set_error(const lldb::SBError &error) {
  const char *text = error.GetCString();
  set_error(text && text[0] ? text : "the LLDB operation failed");
}

void copy(char *dst, size_t cap, const char *src) {
  if (!dst || cap == 0) {
    return;
  }
  if (!src) {
    dst[0] = '\0';
    return;
  }
  std::snprintf(dst, cap, "%s", src);
}

int map_state(lldb::StateType state) {
  switch (state) {
    case lldb::eStateStopped:
    case lldb::eStateCrashed:
    case lldb::eStateSuspended:
      return COVE_STATE_STOPPED;
    case lldb::eStateRunning:
    case lldb::eStateStepping:
      return COVE_STATE_RUNNING;
    case lldb::eStateAttaching:
    case lldb::eStateLaunching:
    case lldb::eStateConnected:
      return COVE_STATE_ATTACHED;
    case lldb::eStateExited:
      return COVE_STATE_EXITED;
    case lldb::eStateDetached:
    case lldb::eStateUnloaded:
      return COVE_STATE_TERMINATED;
    default:
      return COVE_STATE_STARTING;
  }
}

int map_stop_reason(lldb::StopReason reason) {
  switch (reason) {
    case lldb::eStopReasonBreakpoint:
      return COVE_STOP_BREAKPOINT;
    case lldb::eStopReasonTrace:
    case lldb::eStopReasonPlanComplete:
      return COVE_STOP_STEP;
    case lldb::eStopReasonSignal:
      return COVE_STOP_SIGNAL;
    case lldb::eStopReasonException:
      return COVE_STOP_EXCEPTION;
    case lldb::eStopReasonWatchpoint:
      return COVE_STOP_WATCHPOINT;
    case lldb::eStopReasonNone:
      return COVE_STOP_NONE;
    default:
      return COVE_STOP_OTHER;
  }
}

bool has_stop_reason(lldb::SBThread &thread) {
  if (!thread.IsValid()) {
    return false;
  }
  lldb::StopReason reason = thread.GetStopReason();
  return reason != lldb::eStopReasonNone && reason != lldb::eStopReasonInvalid;
}

struct Session;

/* Baton for a log point. Kept until the session is destroyed: LLDB may still be
 * inside the callback when the breakpoint is deleted. */
struct LogPoint {
  Session *session;
  std::string message;
};

enum class ValueKind { List, Parent, Statics, Registers };

struct ValueEntry {
  ValueKind kind;
  std::vector<lldb::SBValue> values;
  lldb::SBFrame frame;
};

struct Output {
  int category;
  std::string text;
};

struct Session {
  lldb::SBDebugger debugger;
  lldb::SBTarget target;
  lldb::SBProcess process;
  lldb::SBListener listener;
  lldb::SBBroadcaster waker;
  std::mutex mutex;
  cove_state state{};
  std::vector<lldb::SBFrame> frames;
  std::vector<ValueEntry> values;
  std::map<std::string, lldb::break_id_t> exception_filters;
  std::vector<std::unique_ptr<LogPoint>> log_points;
  bool attached = false;

  /* Written from LLDB's breakpoint callback thread, so it has its own lock. */
  std::mutex output_mutex;
  std::deque<Output> output;

  void clear_handles() {
    frames.clear();
    values.clear();
  }

  cove_handle add_frame(const lldb::SBFrame &frame) {
    frames.push_back(frame);
    return static_cast<cove_handle>(frames.size());
  }

  cove_handle add_entry(ValueEntry entry) {
    values.push_back(std::move(entry));
    return static_cast<cove_handle>(values.size());
  }

  lldb::SBFrame frame(cove_handle id) {
    if (id == 0 || id > frames.size()) {
      return lldb::SBFrame();
    }
    return frames[static_cast<size_t>(id - 1)];
  }

  void push_output(int category, std::string text) {
    if (text.empty()) {
      return;
    }
    std::lock_guard<std::mutex> lock(output_mutex);
    output.push_back(Output{category, std::move(text)});
    while (output.size() > kMaxOutput) {
      output.pop_front();
    }
  }

  bool is_exception_breakpoint(lldb::break_id_t id) {
    for (const auto &entry : exception_filters) {
      if (entry.second == id) {
        return true;
      }
    }
    return false;
  }
};

Session *as_session(cove_session handle) { return static_cast<Session *>(handle); }

/* A private listener does not get LLDB's default "select the thread that
 * stopped" behaviour, so it is done here. */
lldb::SBThread stopped_thread(Session *session) {
  lldb::SBThread selected = session->process.GetSelectedThread();
  if (has_stop_reason(selected)) {
    return selected;
  }
  uint32_t count = session->process.GetNumThreads();
  for (uint32_t i = 0; i < count; ++i) {
    lldb::SBThread thread = session->process.GetThreadAtIndex(i);
    if (has_stop_reason(thread)) {
      session->process.SetSelectedThread(thread);
      return thread;
    }
  }
  return selected;
}

void fold_stop(Session *session) {
  session->state.stop_count += 1;
  session->state.num_breakpoints = 0;
  session->state.description[0] = '\0';
  session->state.stop_reason = COVE_STOP_NONE;
  session->state.thread_id = 0;
  lldb::SBThread thread = stopped_thread(session);
  if (!thread.IsValid()) {
    return;
  }
  session->state.thread_id = thread.GetThreadID();
  lldb::StopReason reason = thread.GetStopReason();
  session->state.stop_reason = map_stop_reason(reason);
  thread.GetStopDescription(session->state.description,
                            sizeof(session->state.description));
  if (reason == lldb::eStopReasonBreakpoint) {
    size_t count = thread.GetStopReasonDataCount();
    for (size_t i = 0; i + 1 < count && session->state.num_breakpoints < 8; i += 2) {
      uint64_t id = thread.GetStopReasonDataAtIndex(static_cast<uint32_t>(i));
      session->state.breakpoint_ids[session->state.num_breakpoints++] = id;
      if (session->is_exception_breakpoint(static_cast<lldb::break_id_t>(id))) {
        session->state.stop_reason = COVE_STOP_EXCEPTION;
      }
    }
  }
}

void fold_state(Session *session, lldb::StateType state) {
  session->state.state = map_state(state);
  lldb::pid_t pid = session->process.GetProcessID();
  if (pid != LLDB_INVALID_PROCESS_ID) {
    session->state.pid = static_cast<uint32_t>(pid);
  }
  lldb::SBFileSpec exe = session->target.GetExecutable();
  copy(session->state.process_name, sizeof(session->state.process_name),
       exe.GetFilename());
  if (state == lldb::eStateStopped || state == lldb::eStateCrashed ||
      state == lldb::eStateSuspended) {
    fold_stop(session);
  } else {
    session->clear_handles();
    session->state.stop_reason = COVE_STOP_NONE;
    session->state.thread_id = 0;
    session->state.num_breakpoints = 0;
    session->state.description[0] = '\0';
    if (state == lldb::eStateExited) {
      session->state.exit_code = session->process.GetExitStatus();
      session->state.has_exit_code = 1;
      copy(session->state.description, sizeof(session->state.description),
           session->process.GetExitDescription());
    }
  }
}

lldb::SBThread thread_by_id(Session *session, uint64_t thread_id) {
  if (thread_id != 0) {
    lldb::SBThread thread = session->process.GetThreadByID(thread_id);
    if (thread.IsValid()) {
      return thread;
    }
  }
  return session->process.GetSelectedThread();
}

std::string display_value(lldb::SBValue &value) {
  lldb::SBError error = value.GetError();
  if (error.Fail()) {
    const char *text = error.GetCString();
    return std::string("<error: ") + (text ? text : "unreadable") + ">";
  }
  std::string text;
  if (const char *raw = value.GetValue()) {
    text = raw;
  }
  const char *summary = value.GetSummary();
  if (summary && summary[0]) {
    if (!text.empty()) {
      text.push_back(' ');
    }
    text += summary;
  }
  if (text.empty() && value.MightHaveChildren()) {
    text = "{...}";
  }
  return text;
}

/* Must be called with session->mutex held: it may add a value handle. */
void fill_variable(cove_variable *out, lldb::SBValue &value, Session *session) {
  std::memset(out, 0, sizeof(*out));
  copy(out->name, sizeof(out->name), value.GetName());
  std::string text = display_value(value);
  copy(out->value, sizeof(out->value), text.c_str());
  out->value_truncated = text.size() >= sizeof(out->value);
  copy(out->type, sizeof(out->type), value.GetTypeName());
  lldb::SBStream path;
  if (value.GetExpressionPath(path) && path.GetData()) {
    copy(out->evaluate_name, sizeof(out->evaluate_name), path.GetData());
  }
  lldb::addr_t address = value.GetLoadAddress();
  if (address != LLDB_INVALID_ADDRESS) {
    out->address = address;
    out->has_address = 1;
  }
  if (value.MightHaveChildren()) {
    uint32_t children = value.GetNumChildren();
    out->num_children = children;
    if (children > 0) {
      out->ref = session->add_entry(ValueEntry{ValueKind::Parent, {value}, {}});
    }
  }
}

std::vector<lldb::SBValue> to_vector(lldb::SBValueList list) {
  std::vector<lldb::SBValue> values;
  uint32_t size = list.GetSize();
  values.reserve(size);
  for (uint32_t i = 0; i < size; ++i) {
    values.push_back(list.GetValueAtIndex(i));
  }
  return values;
}

lldb::SBValue evaluate_in(lldb::SBFrame &frame, const char *expression,
                          uint32_t timeout_ms) {
  /* A variable path reads memory without running the debuggee. */
  lldb::SBValue value = frame.GetValueForVariablePath(expression);
  if (value.IsValid() && value.GetError().Success()) {
    return value;
  }
  lldb::SBExpressionOptions options;
  options.SetUnwindOnError(true);
  options.SetIgnoreBreakpoints(true);
  options.SetTryAllThreads(true);
  if (timeout_ms > 0) {
    options.SetTimeoutInMicroSeconds(timeout_ms * 1000u);
  }
  return frame.EvaluateExpression(expression, options);
}

/* Replaces each {expression} in a log message with its value in `frame`. */
std::string interpolate(const std::string &message, lldb::SBFrame &frame) {
  std::string out;
  size_t i = 0;
  while (i < message.size()) {
    char c = message[i];
    if (c == '\\' && i + 1 < message.size() &&
        (message[i + 1] == '{' || message[i + 1] == '}')) {
      out.push_back(message[i + 1]);
      i += 2;
      continue;
    }
    if (c != '{') {
      out.push_back(c);
      ++i;
      continue;
    }
    size_t close = message.find('}', i + 1);
    if (close == std::string::npos) {
      out.append(message, i, std::string::npos);
      break;
    }
    std::string expression = message.substr(i + 1, close - i - 1);
    if (expression.empty() || !frame.IsValid()) {
      out.append(message, i, close - i + 1);
    } else {
      lldb::SBValue value = evaluate_in(frame, expression.c_str(), 500);
      out += value.IsValid() ? display_value(value) : "<error: no value>";
    }
    i = close + 1;
  }
  return out;
}

bool log_point_hit(void *baton, lldb::SBProcess &, lldb::SBThread &thread,
                   lldb::SBBreakpointLocation &) {
  auto *log_point = static_cast<LogPoint *>(baton);
  lldb::SBFrame frame = thread.GetFrameAtIndex(0);
  log_point->session->push_output(COVE_OUTPUT_LOG,
                                  interpolate(log_point->message, frame));
  return false;
}

void drain_stdio(Session *session, bool error_stream) {
  char buffer[1000];
  for (;;) {
    size_t read = error_stream
                      ? session->process.GetSTDERR(buffer, sizeof(buffer))
                      : session->process.GetSTDOUT(buffer, sizeof(buffer));
    if (read == 0) {
      break;
    }
    session->push_output(error_stream ? COVE_OUTPUT_STDERR : COVE_OUTPUT_STDOUT,
                         std::string(buffer, read));
  }
}

void breakpoint_resolution(lldb::SBBreakpoint &breakpoint, uint32_t fallback,
                           int *out_verified, uint32_t *out_resolved_line) {
  size_t locations = breakpoint.GetNumLocations();
  if (out_verified) {
    *out_verified = locations > 0 ? 1 : 0;
  }
  if (out_resolved_line) {
    *out_resolved_line = fallback;
    if (locations > 0) {
      lldb::SBLineEntry entry =
          breakpoint.GetLocationAtIndex(0).GetAddress().GetLineEntry();
      if (entry.IsValid() && entry.GetLine() != 0) {
        *out_resolved_line = entry.GetLine();
      }
    }
  }
}

}  // namespace

extern "C" {

COVE_LLDB_API int cove_lldb_initialize(void) {
  static std::once_flag once;
  std::call_once(once, [] { lldb::SBDebugger::Initialize(); });
  return COVE_LLDB_OK;
}

COVE_LLDB_API const char *cove_lldb_version(void) {
  /* LLDB builds its version string lazily without a lock, so it is read once,
   * after initialization, and cached. */
  static std::once_flag once;
  static std::string version;
  cove_lldb_initialize();
  std::call_once(once, [] {
    const char *text = lldb::SBDebugger::GetVersionString();
    version = text ? text : "";
  });
  return version.c_str();
}

COVE_LLDB_API const char *cove_lldb_last_error(void) { return g_error.c_str(); }

COVE_LLDB_API cove_session cove_lldb_session_create(void) {
  auto *session = new Session();
  session->debugger = lldb::SBDebugger::Create(false);
  if (!session->debugger.IsValid()) {
    set_error("cannot create an LLDB debugger");
    delete session;
    return nullptr;
  }
  session->debugger.SetAsync(true);
  session->listener = lldb::SBListener("cove.debugger");
  session->waker = lldb::SBBroadcaster("cove.debugger.wake");
  session->waker.AddListener(session->listener, kWakeEvent);
  session->state.state = COVE_STATE_STARTING;
  return session;
}

COVE_LLDB_API void cove_lldb_session_destroy(cove_session handle) {
  Session *session = as_session(handle);
  if (!session) {
    return;
  }
  if (session->debugger.IsValid()) {
    lldb::SBDebugger::Destroy(session->debugger);
  }
  delete session;
}

COVE_LLDB_API int cove_lldb_attach(cove_session handle, uint32_t pid) {
  Session *session = as_session(handle);
  if (!session) {
    return COVE_LLDB_ERR;
  }
  lldb::SBError error;
  lldb::SBTarget target =
      session->debugger.CreateTarget("", nullptr, nullptr, true, error);
  if (!target.IsValid()) {
    set_error(error);
    return COVE_LLDB_ERR;
  }
  lldb::SBProcess process =
      target.AttachToProcessWithID(session->listener, pid, error);
  if (error.Fail() || !process.IsValid()) {
    set_error(error);
    return COVE_LLDB_ERR;
  }
  std::lock_guard<std::mutex> lock(session->mutex);
  session->target = target;
  session->process = process;
  session->attached = true;
  session->state.pid = pid;
  fold_state(session, process.GetState());
  return COVE_LLDB_OK;
}

COVE_LLDB_API int cove_lldb_attach_remote(cove_session handle, const char *url,
                                        const char *program) {
  Session *session = as_session(handle);
  if (!session || !url) {
    return COVE_LLDB_ERR;
  }
  lldb::SBError error;
  lldb::SBTarget target = session->debugger.CreateTarget(
      program && program[0] ? program : "", nullptr, nullptr, true, error);
  if (!target.IsValid()) {
    set_error(error);
    return COVE_LLDB_ERR;
  }
  lldb::SBProcess process =
      target.ConnectRemote(session->listener, url, "gdb-remote", error);
  if (error.Fail() || !process.IsValid()) {
    set_error(error);
    return COVE_LLDB_ERR;
  }
  std::lock_guard<std::mutex> lock(session->mutex);
  session->target = target;
  session->process = process;
  session->attached = true;
  fold_state(session, process.GetState());
  return COVE_LLDB_OK;
}

COVE_LLDB_API int cove_lldb_detach(cove_session handle) {
  Session *session = as_session(handle);
  if (!session) {
    return COVE_LLDB_ERR;
  }
  lldb::SBProcess process;
  {
    std::lock_guard<std::mutex> lock(session->mutex);
    process = session->process;
    session->attached = false;
  }
  lldb::SBError error;
  if (process.IsValid()) {
    error = process.Detach(false);
  }
  std::lock_guard<std::mutex> lock(session->mutex);
  session->clear_handles();
  session->state.state = COVE_STATE_CLOSED;
  if (error.Fail()) {
    set_error(error);
    return COVE_LLDB_ERR;
  }
  return COVE_LLDB_OK;
}

COVE_LLDB_API void cove_lldb_abort(cove_session handle) {
  Session *session = as_session(handle);
  if (!session) {
    return;
  }
  bool attached;
  lldb::SBProcess process;
  {
    std::lock_guard<std::mutex> lock(session->mutex);
    attached = session->attached;
    process = session->process;
    session->attached = false;
  }
  if (attached && process.IsValid()) {
    process.Detach(false);
  }
  std::lock_guard<std::mutex> lock(session->mutex);
  session->clear_handles();
  session->state.state = COVE_STATE_CLOSED;
}

COVE_LLDB_API int cove_lldb_continue(cove_session handle) {
  Session *session = as_session(handle);
  if (!session || !session->process.IsValid()) {
    set_error("no process to continue");
    return COVE_LLDB_ERR;
  }
  {
    std::lock_guard<std::mutex> lock(session->mutex);
    session->clear_handles();
  }
  lldb::SBError error = session->process.Continue();
  if (error.Fail()) {
    set_error(error);
    return COVE_LLDB_ERR;
  }
  return COVE_LLDB_OK;
}

COVE_LLDB_API int cove_lldb_pause(cove_session handle) {
  Session *session = as_session(handle);
  if (!session || !session->process.IsValid()) {
    set_error("no process to pause");
    return COVE_LLDB_ERR;
  }
  lldb::SBError error = session->process.Stop();
  if (error.Fail()) {
    set_error(error);
    return COVE_LLDB_ERR;
  }
  return COVE_LLDB_OK;
}

COVE_LLDB_API int cove_lldb_step(cove_session handle, uint64_t thread_id,
                               int kind) {
  Session *session = as_session(handle);
  if (!session || !session->process.IsValid()) {
    set_error("no process to step");
    return COVE_LLDB_ERR;
  }
  lldb::SBThread thread = thread_by_id(session, thread_id);
  if (!thread.IsValid()) {
    set_error("no thread to step");
    return COVE_LLDB_ERR;
  }
  {
    std::lock_guard<std::mutex> lock(session->mutex);
    session->clear_handles();
  }
  lldb::SBError error;
  switch (kind) {
    case COVE_STEP_IN:
      thread.StepInto(nullptr, LLDB_INVALID_LINE_NUMBER, error,
                      lldb::eOnlyDuringStepping);
      break;
    case COVE_STEP_OUT:
      thread.StepOut(error);
      break;
    default:
      thread.StepOver(lldb::eOnlyDuringStepping, error);
      break;
  }
  if (error.Fail()) {
    set_error(error);
    return COVE_LLDB_ERR;
  }
  return COVE_LLDB_OK;
}

COVE_LLDB_API int cove_lldb_get_state(cove_session handle, cove_state *out) {
  Session *session = as_session(handle);
  if (!session || !out) {
    return COVE_LLDB_ERR;
  }
  std::lock_guard<std::mutex> lock(session->mutex);
  *out = session->state;
  return COVE_LLDB_OK;
}

COVE_LLDB_API int cove_lldb_wait_event(cove_session handle, uint32_t timeout_ms) {
  Session *session = as_session(handle);
  if (!session) {
    return 0;
  }
  lldb::SBEvent event;
  bool got;
  if (timeout_ms == 0) {
    got = session->listener.GetNextEvent(event);
  } else {
    uint32_t seconds = (timeout_ms + 999) / 1000;
    got = session->listener.WaitForEvent(seconds, event);
  }
  if (!got || !event.IsValid()) {
    return 0;
  }
  if (!lldb::SBProcess::EventIsProcessEvent(event)) {
    return 1;
  }
  uint32_t type = event.GetType();
  if (type & lldb::SBProcess::eBroadcastBitSTDOUT) {
    drain_stdio(session, false);
  }
  if (type & lldb::SBProcess::eBroadcastBitSTDERR) {
    drain_stdio(session, true);
  }
  if (!(type & lldb::SBProcess::eBroadcastBitStateChanged)) {
    return 1;
  }
  lldb::StateType state = lldb::SBProcess::GetStateFromEvent(event);
  if (state == lldb::eStateInvalid) {
    return 1;
  }
  /* An auto-continued stop (a log point, a false condition) is not a stop. */
  if (state == lldb::eStateStopped &&
      lldb::SBProcess::GetRestartedFromEvent(event)) {
    return 1;
  }
  std::lock_guard<std::mutex> lock(session->mutex);
  if (session->state.state == COVE_STATE_CLOSED) {
    return 1;
  }
  fold_state(session, state);
  return 1;
}

COVE_LLDB_API void cove_lldb_wake(cove_session handle) {
  Session *session = as_session(handle);
  if (session) {
    session->waker.BroadcastEventByType(kWakeEvent);
  }
}

COVE_LLDB_API int cove_lldb_drain_output(cove_session handle, cove_output *buf,
                                       int max, int *out_count) {
  Session *session = as_session(handle);
  if (out_count) {
    *out_count = 0;
  }
  if (!session || !buf || max <= 0) {
    return COVE_LLDB_ERR;
  }
  std::lock_guard<std::mutex> lock(session->output_mutex);
  int written = 0;
  while (written < max && !session->output.empty()) {
    Output &entry = session->output.front();
    buf[written].category = entry.category;
    copy(buf[written].text, sizeof(buf[written].text), entry.text.c_str());
    session->output.pop_front();
    ++written;
  }
  if (out_count) {
    *out_count = written;
  }
  return COVE_LLDB_OK;
}

COVE_LLDB_API int cove_lldb_threads(cove_session handle, cove_thread *buf,
                                  int max, int *out_count) {
  Session *session = as_session(handle);
  if (out_count) {
    *out_count = 0;
  }
  if (!session || !session->process.IsValid() || !buf || max <= 0) {
    set_error("no process to list threads for");
    return COVE_LLDB_ERR;
  }
  uint32_t count = session->process.GetNumThreads();
  int written = 0;
  for (uint32_t i = 0; i < count && written < max; ++i) {
    lldb::SBThread thread = session->process.GetThreadAtIndex(i);
    if (!thread.IsValid()) {
      continue;
    }
    buf[written].id = thread.GetThreadID();
    copy(buf[written].name, sizeof(buf[written].name), thread.GetName());
    ++written;
  }
  if (out_count) {
    *out_count = written;
  }
  return COVE_LLDB_OK;
}

COVE_LLDB_API int cove_lldb_stack(cove_session handle, uint64_t thread_id,
                                uint64_t start, uint64_t levels,
                                cove_frame *buf, int max, uint64_t *out_total) {
  Session *session = as_session(handle);
  if (out_total) {
    *out_total = 0;
  }
  if (!session || !session->process.IsValid() || !buf || max <= 0) {
    set_error("no process to read a stack from");
    return COVE_LLDB_ERR;
  }
  lldb::SBThread thread = thread_by_id(session, thread_id);
  if (!thread.IsValid()) {
    set_error("no thread for the stack trace");
    return COVE_LLDB_ERR;
  }
  uint32_t total = thread.GetNumFrames();
  if (out_total) {
    *out_total = total;
  }
  std::lock_guard<std::mutex> lock(session->mutex);
  uint64_t written = 0;
  for (uint64_t i = start;
       i < total && written < static_cast<uint64_t>(max) && written < levels; ++i) {
    lldb::SBFrame frame = thread.GetFrameAtIndex(static_cast<uint32_t>(i));
    if (!frame.IsValid()) {
      continue;
    }
    cove_frame *out = &buf[written];
    std::memset(out, 0, sizeof(*out));
    out->id = session->add_frame(frame);
    const char *name = frame.GetDisplayFunctionName();
    copy(out->name, sizeof(out->name), name ? name : frame.GetFunctionName());
    lldb::SBLineEntry entry = frame.GetLineEntry();
    if (entry.IsValid()) {
      out->line = entry.GetLine();
      out->column = entry.GetColumn();
      entry.GetFileSpec().GetPath(out->path, sizeof(out->path));
    }
    std::snprintf(out->pc, sizeof(out->pc), "0x%llx",
                  static_cast<unsigned long long>(frame.GetPC()));
    ++written;
  }
  return COVE_LLDB_OK;
}

COVE_LLDB_API int cove_lldb_scopes(cove_session handle, cove_handle frame_id,
                                 cove_scope *buf, int max, int *out_count) {
  Session *session = as_session(handle);
  if (out_count) {
    *out_count = 0;
  }
  if (!session || !buf || max <= 0) {
    return COVE_LLDB_ERR;
  }
  std::lock_guard<std::mutex> lock(session->mutex);
  lldb::SBFrame frame = session->frame(frame_id);
  if (!frame.IsValid()) {
    set_error("the frame handle is no longer valid");
    return COVE_LLDB_ERR;
  }
  int written = 0;
  auto emit = [&](const char *name, ValueEntry entry, bool expensive) {
    if (written >= max) {
      return;
    }
    buf[written].ref = session->add_entry(std::move(entry));
    copy(buf[written].name, sizeof(buf[written].name), name);
    buf[written].expensive = expensive ? 1 : 0;
    ++written;
  };
  std::vector<lldb::SBValue> arguments =
      to_vector(frame.GetVariables(true, false, false, true));
  if (!arguments.empty()) {
    emit("Arguments", ValueEntry{ValueKind::List, std::move(arguments), {}}, false);
  }
  emit("Locals",
       ValueEntry{ValueKind::List,
                  to_vector(frame.GetVariables(false, true, false, true)), {}},
       false);
  /* Statics and registers are read only when the scope is expanded. */
  emit("Statics", ValueEntry{ValueKind::Statics, {}, frame}, true);
  emit("Registers", ValueEntry{ValueKind::Registers, {}, frame}, true);
  if (out_count) {
    *out_count = written;
  }
  return COVE_LLDB_OK;
}

COVE_LLDB_API int cove_lldb_variables(cove_session handle, cove_handle ref,
                                    uint64_t start, uint64_t count,
                                    cove_variable *buf, int max,
                                    uint64_t *out_total) {
  Session *session = as_session(handle);
  if (out_total) {
    *out_total = 0;
  }
  if (!session || !buf || max <= 0 || ref == 0) {
    return COVE_LLDB_ERR;
  }
  std::lock_guard<std::mutex> lock(session->mutex);
  if (ref > session->values.size()) {
    set_error("the variables reference is no longer valid");
    return COVE_LLDB_ERR;
  }
  size_t index = static_cast<size_t>(ref - 1);
  ValueKind kind = session->values[index].kind;
  if (kind == ValueKind::Statics || kind == ValueKind::Registers) {
    lldb::SBFrame frame = session->values[index].frame;
    std::vector<lldb::SBValue> resolved =
        kind == ValueKind::Statics
            ? to_vector(frame.GetVariables(false, false, true, true))
            : to_vector(frame.GetRegisters());
    session->values[index].values = std::move(resolved);
    session->values[index].kind = ValueKind::List;
    kind = ValueKind::List;
  }
  uint64_t limit = count < static_cast<uint64_t>(max) ? count : static_cast<uint64_t>(max);
  uint64_t written = 0;
  if (kind == ValueKind::List) {
    /* Copy: fill_variable appends to session->values and may reallocate it. */
    std::vector<lldb::SBValue> children = session->values[index].values;
    if (out_total) {
      *out_total = children.size();
    }
    for (uint64_t i = start; i < children.size() && written < limit; ++i) {
      fill_variable(&buf[written], children[static_cast<size_t>(i)], session);
      ++written;
    }
    return COVE_LLDB_OK;
  }
  if (session->values[index].values.empty()) {
    return COVE_LLDB_OK;
  }
  lldb::SBValue parent = session->values[index].values[0];
  uint64_t total = parent.GetNumChildren();
  if (out_total) {
    *out_total = total;
  }
  for (uint64_t i = start; i < total && written < limit; ++i) {
    lldb::SBValue child = parent.GetChildAtIndex(static_cast<uint32_t>(i));
    fill_variable(&buf[written], child, session);
    ++written;
  }
  return COVE_LLDB_OK;
}

COVE_LLDB_API int cove_lldb_evaluate(cove_session handle, const char *expression,
                                   cove_handle frame_id, uint32_t timeout_ms,
                                   cove_variable *out) {
  Session *session = as_session(handle);
  if (!session || !out || !expression) {
    return COVE_LLDB_ERR;
  }
  lldb::SBFrame frame;
  {
    std::lock_guard<std::mutex> lock(session->mutex);
    frame = session->frame(frame_id);
  }
  if (!frame.IsValid()) {
    if (frame_id != 0) {
      set_error("the frame handle is no longer valid");
      return COVE_LLDB_ERR;
    }
    frame = session->process.GetSelectedThread().GetFrameAtIndex(0);
  }
  if (!frame.IsValid()) {
    set_error("no frame to evaluate in");
    return COVE_LLDB_ERR;
  }
  /* The expression may run the debuggee, so the session lock is not held. */
  lldb::SBValue value = evaluate_in(frame, expression, timeout_ms);
  if (!value.IsValid()) {
    set_error("the expression produced no value");
    return COVE_LLDB_ERR;
  }
  lldb::SBError error = value.GetError();
  if (error.Fail()) {
    set_error(error);
    return COVE_LLDB_ERR;
  }
  std::lock_guard<std::mutex> lock(session->mutex);
  fill_variable(out, value, session);
  return COVE_LLDB_OK;
}

COVE_LLDB_API int cove_lldb_breakpoint_create(cove_session handle, const char *path,
                                            uint32_t line, const char *condition,
                                            const char *log_message,
                                            uint64_t *out_id, int *out_verified,
                                            uint32_t *out_resolved_line) {
  Session *session = as_session(handle);
  if (out_id) {
    *out_id = 0;
  }
  if (out_verified) {
    *out_verified = 0;
  }
  if (out_resolved_line) {
    *out_resolved_line = 0;
  }
  if (!session || !session->target.IsValid() || !path) {
    set_error("no target to set a breakpoint in");
    return COVE_LLDB_ERR;
  }
  lldb::SBBreakpoint breakpoint =
      session->target.BreakpointCreateByLocation(path, line);
  if (!breakpoint.IsValid()) {
    set_error("cannot create the breakpoint");
    return COVE_LLDB_ERR;
  }
  if (condition && condition[0]) {
    breakpoint.SetCondition(condition);
  }
  if (log_message && log_message[0]) {
    std::lock_guard<std::mutex> lock(session->mutex);
    session->log_points.push_back(
        std::unique_ptr<LogPoint>(new LogPoint{session, log_message}));
    breakpoint.SetCallback(log_point_hit, session->log_points.back().get());
  }
  if (out_id) {
    *out_id = static_cast<uint64_t>(breakpoint.GetID());
  }
  breakpoint_resolution(breakpoint, line, out_verified, out_resolved_line);
  return COVE_LLDB_OK;
}

COVE_LLDB_API int cove_lldb_breakpoint_delete(cove_session handle, uint64_t id) {
  Session *session = as_session(handle);
  if (!session || !session->target.IsValid()) {
    set_error("no target to delete a breakpoint from");
    return COVE_LLDB_ERR;
  }
  if (!session->target.BreakpointDelete(static_cast<lldb::break_id_t>(id))) {
    set_error("cannot delete the breakpoint");
    return COVE_LLDB_ERR;
  }
  return COVE_LLDB_OK;
}

COVE_LLDB_API int cove_lldb_breakpoint_info(cove_session handle, uint64_t id,
                                          int *out_verified,
                                          uint32_t *out_resolved_line) {
  Session *session = as_session(handle);
  if (out_verified) {
    *out_verified = 0;
  }
  if (out_resolved_line) {
    *out_resolved_line = 0;
  }
  if (!session || !session->target.IsValid()) {
    return COVE_LLDB_ERR;
  }
  lldb::SBBreakpoint breakpoint =
      session->target.FindBreakpointByID(static_cast<lldb::break_id_t>(id));
  if (!breakpoint.IsValid()) {
    set_error("the breakpoint no longer exists");
    return COVE_LLDB_ERR;
  }
  breakpoint_resolution(breakpoint, 0, out_verified, out_resolved_line);
  return COVE_LLDB_OK;
}

COVE_LLDB_API int cove_lldb_set_exception_filter(cove_session handle,
                                               const char *filter, int enabled) {
  Session *session = as_session(handle);
  if (!session || !session->target.IsValid() || !filter) {
    set_error("no target to set an exception filter in");
    return COVE_LLDB_ERR;
  }
  std::string name = filter;
  std::lock_guard<std::mutex> lock(session->mutex);
  auto existing = session->exception_filters.find(name);
  if (!enabled) {
    if (existing != session->exception_filters.end()) {
      session->target.BreakpointDelete(existing->second);
      session->exception_filters.erase(existing);
    }
    return COVE_LLDB_OK;
  }
  if (existing != session->exception_filters.end()) {
    return COVE_LLDB_OK;
  }
  lldb::SBBreakpoint breakpoint;
  if (name == "cpp_throw") {
    breakpoint = session->target.BreakpointCreateForException(
        lldb::eLanguageTypeC_plus_plus, false, true);
  } else if (name == "cpp_catch") {
    breakpoint = session->target.BreakpointCreateForException(
        lldb::eLanguageTypeC_plus_plus, true, false);
  } else if (name == "objc_throw") {
    breakpoint = session->target.BreakpointCreateForException(
        lldb::eLanguageTypeObjC, false, true);
  } else if (name == "objc_catch") {
    breakpoint = session->target.BreakpointCreateForException(
        lldb::eLanguageTypeObjC, true, false);
  } else if (name == "rust_panic") {
    breakpoint = session->target.BreakpointCreateByName("rust_panic");
  } else {
    set_error("unknown exception filter");
    return COVE_LLDB_ERR;
  }
  if (!breakpoint.IsValid()) {
    set_error("cannot create the exception breakpoint");
    return COVE_LLDB_ERR;
  }
  session->exception_filters[name] = breakpoint.GetID();
  return COVE_LLDB_OK;
}

}  // extern "C"
