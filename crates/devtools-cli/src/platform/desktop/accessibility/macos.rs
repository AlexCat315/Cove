use super::{Entry, LIMIT, MAX_DEPTH, Request, respond};
use cove_devtools_protocol::control::Fault;
use objc2_application_services::{AXError, AXIsProcessTrusted, AXUIElement};
use objc2_core_foundation::{CFArray, CFRetained, CFString, CFType};
use serde_json::Value;
use std::ptr::{self, NonNull};

pub(super) fn execute(pid: u32, _: u32, request: Request<'_>) -> Result<Value, Fault> {
    // SAFETY: the OS returns an accessibility proxy, not an address in the target process.
    if !unsafe { AXIsProcessTrusted() } {
        return Err(Fault::new(
            "accessibility_denied",
            "Grant Accessibility access to the DevTools controller.",
        ));
    }
    let application = unsafe { AXUIElement::new_application(pid as i32) };
    // SAFETY: the live accessibility proxy is retained for the duration of this call.
    unsafe { application.set_messaging_timeout(0.4) };
    let mut nodes = Vec::new();
    let mut truncated = false;
    collect(application, &mut vec![0], None, &mut nodes, &mut truncated);
    respond(request, nodes, truncated, |element| {
        // SAFETY: the element is retained and the action name is a live CFString.
        let result = unsafe { element.perform_action(&CFString::from_str("AXPress")) };
        if result == AXError::Success {
            Ok(())
        } else {
            Err(Fault::new(
                "native_accessibility",
                format!("AXPress failed ({})", result.0),
            ))
        }
    })
}

fn attribute(element: &AXUIElement, name: &str) -> Option<CFRetained<CFType>> {
    let mut value: *const CFType = ptr::null();
    // SAFETY: AX writes one retained CF object to the provided out pointer on success.
    let status = unsafe {
        element.copy_attribute_value(&CFString::from_str(name), NonNull::from(&mut value))
    };
    if status == AXError::Success {
        // SAFETY: the successful copy transferred ownership of this object.
        NonNull::new(value.cast_mut()).map(|value| unsafe { CFRetained::from_raw(value) })
    } else {
        None
    }
}

fn string(element: &AXUIElement, name: &str) -> String {
    attribute(element, name)
        .and_then(|value| value.downcast::<CFString>().ok())
        .map(|value| value.to_string())
        .unwrap_or_default()
}

fn collect(
    element: CFRetained<AXUIElement>,
    path: &mut Vec<usize>,
    parent: Option<&str>,
    nodes: &mut Vec<(Entry, CFRetained<AXUIElement>)>,
    truncated: &mut bool,
) {
    if nodes.len() == LIMIT || path.len() > MAX_DEPTH {
        *truncated = true;
        return;
    }
    let entry = Entry::new(
        path,
        parent,
        string(&element, "AXRole"),
        string(&element, "AXTitle"),
        string(&element, "AXIdentifier"),
    );
    let id = entry.id.clone();
    // AXMenuBar is a separate application attribute and is not always a child.
    let menu = (path.len() == 1)
        .then(|| attribute(&element, "AXMenuBar"))
        .flatten()
        .and_then(|value| value.downcast::<AXUIElement>().ok());
    let children =
        attribute(&element, "AXChildren").and_then(|value| value.downcast::<CFArray>().ok());
    nodes.push((entry, element));
    if let Some(menu) = menu {
        path.push(usize::MAX);
        collect(menu, path, Some(&id), nodes, truncated);
        path.pop();
    }
    if let Some(children) = children {
        // SAFETY: AXChildren is an array of CoreFoundation accessibility objects.
        let children = unsafe { CFRetained::cast_unchecked::<CFArray<CFType>>(children) };
        for (index, child) in children.iter().enumerate() {
            if nodes.len() == LIMIT {
                *truncated = true;
                break;
            }
            if let Ok(child) = child.downcast::<AXUIElement>() {
                path.push(index);
                collect(child, path, Some(&id), nodes, truncated);
                path.pop();
            }
        }
    }
}
