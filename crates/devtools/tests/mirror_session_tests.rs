//! Mirror session wiring: `mirror.start` and `mirror.stop` drive the frame tap
//! and the host's frame-drive hooks, and a mirrored frame reaches the wire.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use cove_core::DrawList;
use cove_core::scene::NodeTree;
use cove_devtools::devtools::DevTools;
use cove_devtools::devtools::mirror::FrameTapPixels;
use cove_devtools::protocol::MirrorStart;
use cove_devtools::server::config::DevToolsConfig;

fn devtools() -> DevTools {
    let config = DevToolsConfig::new("127.0.0.1", 0, "Mirror", true);
    DevTools::new(config, Rc::new(RefCell::new(NodeTree::new())), None, None)
}

fn start() -> MirrorStart {
    MirrorStart {
        fps: Some(60.0),
        quality: Some(0.5),
        scale: Some(1.0),
    }
}

#[test]
fn start_and_stop_drive_the_tap_and_the_host_hooks() {
    let mut devtools = devtools();
    let captures = Rc::new(Cell::new(0_usize));
    let resets = Rc::new(Cell::new(0_usize));
    let counted = captures.clone();
    let reset = resets.clone();
    devtools.attach_frame_tap(
        Rc::new(move |_: &DrawList, width, height, _, _| {
            counted.set(counted.get() + 1);
            Ok(FrameTapPixels::new(
                vec![0_u8; width as usize * height as usize * 4],
                width as usize * 4,
            ))
        }),
        move || reset.set(reset.get() + 1),
    );
    let active = Rc::new(RefCell::new(Vec::new()));
    let log = active.clone();
    *devtools.on_mirror_start.borrow_mut() = Some(Rc::new(move || log.borrow_mut().push(true)));
    let log = active.clone();
    *devtools.on_mirror_stop.borrow_mut() = Some(Rc::new(move || log.borrow_mut().push(false)));
    devtools.start().expect("the server starts");

    let mirror = &devtools.server.host.mirror;
    let (begin, end) = (
        mirror.start.clone().expect("mirror.start is wired"),
        mirror.stop.clone().expect("mirror.stop is wired"),
    );
    assert!(mirror.activate.is_some(), "mirror.activate is wired");
    assert!(!devtools.mirror_is_active());
    assert!(!devtools.mirror_wants_frame(), "an idle tap wants no frame");

    begin(&start());
    assert!(devtools.mirror_is_active(), "mirror.start starts the tap");
    assert!(devtools.mirror_wants_frame());
    devtools.mirror_capture(&DrawList::default(), 8, 8, 8.0, 8.0);
    assert_eq!(captures.get(), 1, "a started tap captures");

    end();
    assert!(!devtools.mirror_is_active(), "mirror.stop stops the tap");
    assert_eq!(resets.get(), 1, "stopping frees the capture target");
    devtools.mirror_capture(&DrawList::default(), 8, 8, 8.0, 8.0);
    assert_eq!(captures.get(), 1, "a stopped tap captures nothing");
    assert_eq!(
        *active.borrow(),
        [true, false],
        "the host hooks run in order"
    );

    devtools.stop();
}

#[test]
fn the_mirror_capability_follows_the_attached_tap() {
    let mut without = devtools();
    without.start().expect("the server starts");
    assert!(
        !without
            .server
            .advertised_capabilities
            .contains(&"mirror".to_owned())
    );
    without.stop();

    let mut with = devtools();
    with.attach_frame_tap(
        Rc::new(|_: &DrawList, _, _, _, _| Ok(FrameTapPixels::new(Vec::new(), 0))),
        || {},
    );
    with.start().expect("the server starts");
    assert!(
        with.server
            .advertised_capabilities
            .contains(&"mirror".to_owned())
    );
    with.stop();
}
