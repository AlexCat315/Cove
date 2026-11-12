use super::{Entry, LIMIT, MAX_DEPTH, Request, respond};
use cove_devtools_protocol::control::Fault;
use serde_json::Value;
use zbus::blocking::{Connection, Proxy, connection::Builder};
use zbus::names::OwnedBusName;
use zbus::zvariant::OwnedObjectPath;

type Object = (OwnedBusName, OwnedObjectPath);

fn error(error: impl std::fmt::Display) -> Fault {
    Fault::new("native_accessibility", error.to_string())
}

pub(super) fn execute(pid: u32, _: u32, request: Request<'_>) -> Result<Value, Fault> {
    let session = Connection::session().map_err(error)?;
    let bus =
        Proxy::new(&session, "org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus").map_err(error)?;
    let address: String = bus.call("GetAddress", &()).map_err(error)?;
    let connection = Builder::address(address.as_str())
        .map_err(error)?
        .build()
        .map_err(error)?;
    let registry = Proxy::new(
        &connection,
        "org.a11y.atspi.Registry",
        "/org/a11y/atspi/accessible/root",
        "org.a11y.atspi.Accessible",
    )
    .map_err(error)?;
    let dbus = Proxy::new(
        &connection,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
    )
    .map_err(error)?;
    let applications: Vec<Object> = registry.call("GetChildren", &()).map_err(error)?;
    let root = applications
        .into_iter()
        .find(|(bus, _)| {
            dbus.call::<_, _, u32>("GetConnectionUnixProcessID", &(bus.as_str(),))
                .ok()
                == Some(pid)
        })
        .ok_or_else(|| {
            Fault::new(
                "unavailable",
                "The application is not registered with AT-SPI.",
            )
        })?;
    let mut nodes = Vec::new();
    let mut truncated = false;
    collect(
        &connection,
        root,
        &mut vec![0],
        None,
        &mut nodes,
        &mut truncated,
    );
    respond(request, nodes, truncated, |(bus, path)| {
        let action = Proxy::new(
            &connection,
            bus.as_str(),
            path.as_str(),
            "org.a11y.atspi.Action",
        )
        .map_err(error)?;
        let actions: Vec<(String, String, String)> =
            action.call("GetActions", &()).map_err(error)?;
        let index = actions
            .iter()
            .position(|(name, _, _)| matches!(name.as_str(), "click" | "press" | "activate"))
            .ok_or_else(|| {
                Fault::new(
                    "unavailable",
                    "The native element has no activation action.",
                )
            })?;
        let ok: bool = action.call("DoAction", &(index as i32,)).map_err(error)?;
        if ok {
            Ok(())
        } else {
            Err(Fault::new(
                "native_accessibility",
                "AT-SPI rejected the action.",
            ))
        }
    })
}

fn collect(
    connection: &Connection,
    object: Object,
    path: &mut Vec<usize>,
    parent: Option<&str>,
    nodes: &mut Vec<(Entry, Object)>,
    truncated: &mut bool,
) {
    if nodes.len() == LIMIT || path.len() > MAX_DEPTH {
        *truncated = true;
        return;
    }
    let Ok(proxy) = Proxy::new(
        connection,
        object.0.as_str(),
        object.1.as_str(),
        "org.a11y.atspi.Accessible",
    ) else {
        return;
    };
    let entry = Entry::new(
        path,
        parent,
        proxy.call("GetRoleName", &()).unwrap_or_default(),
        proxy.get_property("Name").unwrap_or_default(),
        proxy.get_property("AccessibleId").unwrap_or_default(),
    );
    let id = entry.id.clone();
    let children: Vec<Object> = proxy.call("GetChildren", &()).unwrap_or_default();
    drop(proxy);
    nodes.push((entry, object));
    for (index, child) in children.into_iter().enumerate() {
        if nodes.len() == LIMIT {
            *truncated = true;
            break;
        }
        path.push(index);
        collect(connection, child, path, Some(&id), nodes, truncated);
        path.pop();
    }
}
