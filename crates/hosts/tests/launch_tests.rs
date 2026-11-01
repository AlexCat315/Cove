//! Translation of
//! `Hosts/Android/Contract/Tests/CoveAndroidContractTests/LaunchTests.swift`.
//!
//! Behaviour checklist:
//! - `SceneLaunch` round-trips through JSON for every host mode
//! - the encoded document never carries fields outside
//!   `mode`/`component`/`project`

use cove_hosts::android::contract::{SceneLaunch, SceneLaunchMode};

#[test]
fn launch_round_trips_for_every_host_mode() {
    for mode in [
        SceneLaunchMode::Gallery,
        SceneLaunchMode::Counter,
        SceneLaunchMode::Game,
        SceneLaunchMode::Application,
    ] {
        let original = SceneLaunch::new(
            mode,
            "textField",
            (mode == SceneLaunchMode::Game).then(|| "Game".to_owned()),
        );

        let data = serde_json::to_vec(&original).expect("the launch document encodes");
        let decoded = SceneLaunch::from_json(&data).expect("the launch document decodes");
        assert_eq!(decoded, original, "{mode:?} round-trips through JSON");

        let fields: serde_json::Map<String, serde_json::Value> =
            serde_json::from_slice(&data).expect("the document is a JSON object");
        let allowed = ["mode", "component", "project"];
        for key in fields.keys() {
            assert!(
                allowed.contains(&key.as_str()),
                "{mode:?} must not publish `{key}`; allowed keys are {allowed:?}"
            );
        }
    }
}

#[test]
fn launch_mode_serialises_as_lowercase_names() {
    let cases = [
        (SceneLaunchMode::Gallery, "\"gallery\""),
        (SceneLaunchMode::Counter, "\"counter\""),
        (SceneLaunchMode::Game, "\"game\""),
        (SceneLaunchMode::Application, "\"application\""),
    ];
    for (mode, expected) in cases {
        assert_eq!(
            serde_json::to_string(&mode).expect("the mode encodes"),
            expected,
            "the contract's wire name for {mode:?} is stable"
        );
        assert_eq!(mode.as_str(), &expected[1..expected.len() - 1]);
    }
}
