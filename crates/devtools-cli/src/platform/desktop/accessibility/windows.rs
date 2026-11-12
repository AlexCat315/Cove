use super::{Entry, LIMIT, MAX_DEPTH, Request, respond};
use cove_devtools_protocol::control::Fault;
use serde_json::Value;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationInvokePattern,
    IUIAutomationTreeWalker, UIA_InvokePatternId,
};

fn error(error: windows::core::Error) -> Fault {
    Fault::new("native_accessibility", error.to_string())
}

struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: this guard is created only after initialization succeeds on this thread.
        unsafe { CoUninitialize() };
    }
}

pub(super) fn execute(_: u32, window: u32, request: Request<'_>) -> Result<Value, Fault> {
    // SAFETY: COM is initialized on this worker and all interfaces are used and dropped on it.
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }.map_err(error)?;
    let _apartment = Apartment;
    let automation: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }.map_err(error)?;
    // SAFETY: the controller resolves this HWND against the OS window list immediately before calling.
    let root =
        unsafe { automation.ElementFromHandle(HWND(window as usize as *mut _)) }.map_err(error)?;
    let walker = unsafe { automation.ControlViewWalker() }.map_err(error)?;
    let mut nodes = Vec::new();
    let mut truncated = false;
    collect(
        root,
        &walker,
        &mut vec![0],
        None,
        &mut nodes,
        &mut truncated,
    );
    respond(request, nodes, truncated, |element| {
        // SAFETY: the element and invoke pattern belong to the live apartment.
        unsafe {
            if !element.CurrentIsEnabled().map_err(error)?.as_bool() {
                return Err(Fault::new("disabled", "The native element is disabled."));
            }
            let pattern: IUIAutomationInvokePattern = element
                .GetCurrentPatternAs(UIA_InvokePatternId)
                .map_err(error)?;
            pattern.Invoke().map_err(error)
        }
    })
}

fn collect(
    element: IUIAutomationElement,
    walker: &IUIAutomationTreeWalker,
    path: &mut Vec<usize>,
    parent: Option<&str>,
    nodes: &mut Vec<(Entry, IUIAutomationElement)>,
    truncated: &mut bool,
) {
    if nodes.len() == LIMIT || path.len() > MAX_DEPTH {
        *truncated = true;
        return;
    }
    // SAFETY: all UIA objects are valid COM interfaces owned by the current apartment.
    let entry = unsafe {
        Entry::new(
            path,
            parent,
            element
                .CurrentLocalizedControlType()
                .map(|value| value.to_string())
                .unwrap_or_default(),
            element
                .CurrentName()
                .map(|value| value.to_string())
                .unwrap_or_default(),
            element
                .CurrentAutomationId()
                .map(|value| value.to_string())
                .unwrap_or_default(),
        )
    };
    let id = entry.id.clone();
    let mut child = unsafe { walker.GetFirstChildElement(&element) }.ok();
    nodes.push((entry, element));
    let mut index = 0;
    while let Some(current) = child {
        if nodes.len() == LIMIT {
            *truncated = true;
            break;
        }
        child = unsafe { walker.GetNextSiblingElement(&current) }.ok();
        path.push(index);
        collect(current, walker, path, Some(&id), nodes, truncated);
        path.pop();
        index += 1;
    }
}
