//! Software Pet demo: a 2D sprite pet that perches on buttons.
//!
//! Swift source: `Examples/Sources/PetDemo/main.swift`. The Swift file is an
//! entry point that resolves its sprite sheet at launch, so the resource
//! resolution becomes [`resolve_resource_path`] and the run becomes
//! [`pet_demo::main`] rather than a `bin` target.

use cove_core::DrawList;
use cove_ui_compose::pet::{PetPose, PetSpriteSheet};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The sprite display size, in points.
pub const DISPLAY_SIZE: f32 = 124.0;
/// The anchor the pet starts on.
pub const INITIAL_ANCHOR: &str = "home";
/// The window title.
pub const TITLE: &str = "CoveUI · Software Pet";

/// The PNG file stem for a pose, matching `PetPose.rawValue`.
pub fn pose_name(pose: PetPose) -> &'static str {
    match pose {
        PetPose::Idle => "idle",
        PetPose::Windup => "windup",
        PetPose::Rise => "rise",
        PetPose::Apex => "apex",
        PetPose::Fall => "fall",
        PetPose::Land => "land",
        PetPose::Sit => "sit",
    }
}

/// Locates a packaged sprite PNG.
///
/// The Swift original searches `Bundle.main`, then every `.bundle` beside the
/// executable, and finally enumerates each bundle's resources recursively. The
/// Rust image has no `Bundle`, so it searches the same shape of directories on
/// disk and returns the first match, or an empty path when absent.
pub fn resolve_resource_path(name: &str, roots: &[PathBuf]) -> PathBuf {
    let file_name = format!("{name}.png");
    for root in roots {
        let direct = root.join(&file_name);
        if direct.is_file() {
            return direct;
        }
        // Enumerate `.bundle` directories beside the root, as the original does.
        for bundle in child_bundles(root) {
            let candidate = bundle.join(&file_name);
            if candidate.is_file() {
                return candidate;
            }
            if let Some(found) = search_recursively(&bundle, &file_name) {
                return found;
            }
        }
        if let Some(found) = search_recursively(root, &file_name) {
            return found;
        }
    }
    PathBuf::new()
}

fn child_bundles(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut bundles: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().map(|extension| extension == "bundle").unwrap_or(false))
        .collect();
    bundles.sort();
    bundles
}

/// Depth-first search for `file_name`, skipping hidden directories.
fn search_recursively(directory: &Path, file_name: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(directory).ok()?;
    let mut children: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .collect();
    children.sort();
    for path in &children {
        if path.file_name().map(|name| name.to_string_lossy().starts_with('.')).unwrap_or(false) {
            continue;
        }
        if path.is_dir() {
            if let Some(found) = search_recursively(path, file_name) {
                return Some(found);
            }
        } else if path.file_name().map(|name| name == file_name).unwrap_or(false) {
            return Some(path.clone());
        }
    }
    None
}

/// Builds the sprite sheet from whichever frames are present.
///
/// A missing frame is simply absent from the sheet, exactly as the Swift
/// `if !path.isEmpty { frames[pose] = path }` guard leaves it.
pub fn sprite_sheet(name_for: impl Fn(PetPose) -> String) -> PetSpriteSheet {
    let mut frames: HashMap<PetPose, String> = HashMap::new();
    for pose in PetPose::ALL {
        let path = name_for(pose);
        if !path.is_empty() {
            frames.insert(pose, path);
        }
    }
    PetSpriteSheet::new(DISPLAY_SIZE, frames)
}

/// Resolves every frame from the given search roots.
pub fn sprite_sheet_from(roots: &[PathBuf]) -> PetSpriteSheet {
    sprite_sheet(|pose| {
        resolve_resource_path(pose_name(pose), roots)
            .display()
            .to_string()
    })
}

/// The search roots the Swift original uses, in order.
pub fn resource_roots(executable: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(parent) = executable.parent() {
        roots.push(parent.to_path_buf());
        roots.push(parent.join("Resources"));
        if let Some(grandparent) = parent.parent() {
            roots.push(grandparent.to_path_buf());
        }
    }
    roots
}

/// `Examples/Sources/PetDemo/main.swift`.
///
/// Returns the sprite sheet the Swift entry point builds at launch, so a host
/// can render the scene with it.
pub fn main(executable: &Path) -> PetSpriteSheet {
    sprite_sheet_from(&resource_roots(executable))
}

/// The scene's button labels, in the order the Swift demo lays them out.
pub const ACTION_LABELS: [&str; 2] = ["随机游走", "站 / 坐"];
/// The destination anchors the footer buttons target.
pub const ANCHORS: [(&str, &str); 3] = [("首页", "home"), ("设置", "settings"), ("关于", "about")];

/// A painter hook the host supplies for the scene's text.
pub type PetTextPainter = dyn Fn(&str, f32, &mut DrawList, cove_core::Point);

#[cfg(test)]
mod checks {
    use super::*;

    #[test]
    fn every_pose_has_a_distinct_resource_name() {
        let names: Vec<&str> = PetPose::ALL.iter().map(|pose| pose_name(*pose)).collect();
        assert_eq!(
            names,
            vec!["idle", "windup", "rise", "apex", "fall", "land", "sit"],
            "order must match PetPose::ALL so the sheet is deterministic"
        );
        assert_eq!(
            names.iter().collect::<std::collections::BTreeSet<_>>().len(),
            7,
            "no two poses share a frame"
        );
    }

    #[test]
    fn resource_resolution_finds_a_root_level_frame() {
        let root = std::env::temp_dir().join(format!("cove-pet-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("idle.png"), b"x").unwrap();
        assert_eq!(resolve_resource_path("idle", &[root.clone()]), root.join("idle.png"));
        // A missing frame resolves to an empty path rather than panicking.
        assert_eq!(resolve_resource_path("sit", &[root.clone()]), PathBuf::new());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn resource_resolution_searches_beside_bundles() {
        let root = std::env::temp_dir().join(format!("cove-pet-bundle-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let bundle = root.join("PetDemo_PetDemo.bundle");
        std::fs::create_dir_all(&bundle).unwrap();
        std::fs::write(bundle.join("apex.png"), b"x").unwrap();
        assert_eq!(
            resolve_resource_path("apex", &[root.clone()]),
            bundle.join("apex.png")
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn absent_frames_are_omitted_from_the_sheet() {
        let sheet = sprite_sheet(|pose| {
            if pose == PetPose::Idle {
                "/pet/idle.png".to_string()
            } else {
                String::new()
            }
        });
        assert_eq!(sheet.display_size, DISPLAY_SIZE);
        assert_eq!(sheet.frames.len(), 1);
        assert_eq!(sheet.path(PetPose::Idle), Some("/pet/idle.png"));
        assert_eq!(sheet.path(PetPose::Sit), None);
    }

    #[test]
    fn scene_labels_match_the_swift_demo() {
        assert_eq!(ACTION_LABELS, ["随机游走", "站 / 坐"]);
        assert_eq!(
            ANCHORS,
            [("首页", "home"), ("设置", "settings"), ("关于", "about")]
        );
        assert_eq!(INITIAL_ANCHOR, "home");
    }
}
