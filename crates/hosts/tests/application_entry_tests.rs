use cove_composition::{EmptyView, ViewGraph};
use cove_core::scene::{Node, NodeTree, Recomposer};
use cove_hosts::android::contract::SceneLaunchMode;
use cove_hosts::android::sources::cove_android_app::content::scene::{
    DeclaredApplicationContent, HostedSceneContent,
};
use cove_hosts::{ApplicationCheckpoint, ApplicationConfiguration, ApplicationLaunch, NativeMenu};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
};

mod defaults {
    fn content() -> cove_composition::EmptyView {
        cove_composition::EmptyView
    }
    cove_hosts::application!(content);
}

mod configured {
    fn launch() -> cove_hosts::ApplicationLaunch {
        cove_hosts::ApplicationLaunch::new(cove_composition::EmptyView).configure(|app| {
            app.window.title = "Configured application".into();
            app.window.width = 800;
            app.frames.target_rate = Some(30.0);
        })
    }
    cove_hosts::application!(launch = launch);
}

#[test]
fn both_entry_forms_construct_without_starting_a_native_host() {
    let _: fn() -> Result<(), String> = defaults::run;
    let _: fn() -> Result<(), String> = configured::run;
    assert_eq!(
        defaults::application().configuration,
        ApplicationConfiguration::default()
    );
    let launch = configured::application();
    assert_eq!(launch.configuration.window.title, "Configured application");
    assert_eq!(launch.configuration.window.width, 800);
    assert_eq!(
        launch.configuration.frames.validated_target_rate(),
        Some(30.0)
    );
}

#[test]
fn android_preserves_configuration_and_checkpoint_ownership() {
    let value = Rc::new(Cell::new(7));
    let capture = value.clone();
    let restore = value.clone();
    let launch = ApplicationLaunch::new(EmptyView)
        .configuration(ApplicationConfiguration::new(Some("Shared")))
        .checkpoint(ApplicationCheckpoint::new(
            move || [("value".into(), capture.get().to_string())].into(),
            move |values| {
                let Some(value) = values.get("value").and_then(|text| text.parse().ok()) else {
                    return false;
                };
                restore.set(value);
                true
            },
        ));
    let mut content = DeclaredApplicationContent(launch);
    assert_eq!(content.configuration().unwrap().window.title, "Shared");
    content
        .restore_checkpoint(r#"{"value":"42"}"#, SceneLaunchMode::Application)
        .unwrap();
    content.root(SceneLaunchMode::Application, 2.0).unwrap();
    assert_eq!(value.get(), 42);
    assert_eq!(
        content.checkpoint(),
        BTreeMap::from([("value".into(), "42".into())])
    );
    assert!(
        content
            .restore_checkpoint(r#"{"value":"invalid"}"#, SceneLaunchMode::Application)
            .is_err()
    );
    assert_eq!(
        value.get(),
        42,
        "rejected checkpoints preserve authored state"
    );
    assert!(
        content
            .restore_checkpoint("not json", SceneLaunchMode::Application)
            .is_err()
    );
}

#[test]
fn embedded_content_applies_the_wrapper_without_window_chrome() {
    let wrapped = Rc::new(Cell::new(false));
    let observed = wrapped.clone();
    let launch = ApplicationLaunch::new(EmptyView)
        .menu_bar(vec![NativeMenu::named("Navigate").action("Home", || {})])
        .wrap_content(move |content| {
            observed.set(true);
            content
        });
    let tree = Rc::new(RefCell::new(NodeTree::new()));
    let graph = Rc::new(ViewGraph::new(tree.clone(), Rc::new(Recomposer::new())));
    let mut hosted = DeclaredApplicationContent(launch);
    graph.install_view(
        hosted.root(SceneLaunchMode::Application, 1.0).unwrap(),
        Default::default(),
    );
    graph.recomposer.commit_all();

    fn has_menu(node: &Node) -> bool {
        node.accessibility()
            .is_some_and(|semantics| semantics.label == "Navigate")
            || node.children().iter().any(has_menu)
    }

    assert!(wrapped.get());
    assert!(!has_menu(&tree.borrow().root().unwrap()));
}
