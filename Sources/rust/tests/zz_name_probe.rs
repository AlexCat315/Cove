mod workspace_support;
use workspace_support::{controller_with_fixture, WorkspaceRig, GlobalTestLock};

#[test]
fn dump_names() {
    GlobalTestLock::locked(|| {
        let controller = controller_with_fixture();
        let rig = WorkspaceRig::standard(controller.clone(), 1000.0, 640.0);
        rig.pump();
        let mut names: Vec<String> = rig.nodes().iter().filter_map(|n| workspace_support::debug_name(n)).collect();
        names.sort();
        for n in names { eprintln!("NAME {n}"); }
        eprintln!("---- collapse trailing ----");
        rig.collapse("trailing");
        let mut names: Vec<String> = rig.nodes().iter().filter_map(|n| workspace_support::debug_name(n)).collect();
        names.sort();
        for n in names { eprintln!("NAME {n}"); }
    });
}
