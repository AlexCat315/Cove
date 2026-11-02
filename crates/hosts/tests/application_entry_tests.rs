use cove_composition::EmptyView;
use cove_hosts::android::contract::SceneLaunchMode;
use cove_hosts::android::sources::cove_android_app::content::scene::{
    DeclaredApplicationContent, HostedSceneContent,
};
use cove_hosts::{ApplicationCheckpoint, ApplicationConfiguration, ApplicationLaunch};
use std::{cell::Cell, collections::BTreeMap, rc::Rc};

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
