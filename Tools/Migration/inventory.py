#!/usr/bin/env python3
"""Write the source-to-Rust migration inventory. This does not compile or run tests."""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / "Docs/rust-migration-files.json"

CORE_VALUES = {
    "AnalyticClipper": ["analytic_clipper.rs"], "AnalyticStroke": ["analytic_stroke.rs", "shape_kernels.c"],
    "Bidi": ["bidi.rs", "text_kernels.c"], "Binding": ["binding.rs"], "Bitmap": ["bitmap.rs", "draw_list_resources.rs"],
    "Color": ["geometry.rs"], "ContentScale": ["content_scale.rs"], "DrawList": ["draw.rs", "draw_primitives.rs", "shape_kernels.c"],
    "DrawListResources": ["draw_list_resources.rs"], "DynamicProperty": ["dynamic_property.rs"], "Font": ["font.rs"],
    "GeometryKernels": ["geometry_kernels.rs"], "GlyphMetrics": ["glyph_metrics.rs"], "LineBreaks": ["line_breaks.rs"],
    "LinearGradient": ["linear_gradient.rs"], "Mipmaps": ["bitmap.rs"], "RoundedClip": ["rounded_clip.rs", "shape_kernels.c"],
    "ShapedGlyph": ["shaped_glyph.rs"], "State": ["state.rs"], "StateReadTracking": ["state_read_tracking.rs", "state.rs"],
    "SurfaceShadow": ["surface_shadow.rs"], "TextLayout": ["text_layout.rs"], "TextMeasureSlot": ["text_measure_slot.rs"],
    "UIReveal": ["ui_reveal.rs"], "UIShader": ["ui_shader.rs", "ui.wgsl"], "UIVertex": ["draw.rs", "geometry.rs"],
    "Environment": ["adaptive_environment.rs"], "TextScale": ["text_scale.rs"],
}
SCENE_GROUPS = {
    "Scrolling/AxisMotion": "scrolling/animator.rs", "Scrolling/RubberBand": "scrolling/physics.rs",
    "Scrolling/ScrollPhysics": "scrolling/physics.rs", "Scrolling/ScrollSpring": "scrolling/physics.rs",
    "Scrolling/ScrollVelocityTracker": "scrolling/physics.rs", "Scrolling/ScrollAnimator": "scrolling/animator.rs",
    "Scrolling/Reveal": "scrolling/reveal.rs", "Animation/Curves/Steps": "animation/animation.rs",
    "Animation/Curves/Oscillation": "animation/animation.rs", "Animation/Curves/Linear": "animation/linear.rs",
    "Animation/Keyframes/Timeline": "animation/timeline.rs", "Animation/AnimationController": "animation/controller.rs",
    "Animation/ActiveAnimationContext": "animation/context.rs", "Animation/AnimatableProperty": "animatable_property.rs",
    "Animation/AnimatableWrite": "animatable_write.rs", "TextInputClient": "input/text_input.rs",
    "TextInputAttachment": "input/text_input.rs", "ReducedMotion": "environment_hooks.rs", "WindowReveal": "environment_hooks.rs",
    "UIWorkScheduler": "environment_hooks.rs", "Layout/CustomLayout": "layout_node.rs",
    "Layout/LayoutDirection": "environment_hooks.rs", "Layout/Node+LayoutVisibility": "layout_node.rs", "Exports": "mod.rs",
}

def snake(value: str) -> str:
    value = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", value)
    return re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", value).replace("+", "_").replace("-", "_").lower()

def mapped_paths(source: str) -> tuple[str, list[str]]:
    path = Path(source)
    if path.name == "Package.swift":
        cargo = path.with_name("Cargo.toml")
        return ("manifest_written" if (ROOT / cargo).is_file() else "pending", [str(cargo)])
    if "/Tests/" in source or source.startswith("Tests/") or "/Templates/" in source:
        return "pending", []
    if source.startswith("Core/Sources/CoveUICore/"):
        paths = ["Core/rust/" + name for name in CORE_VALUES.get(path.stem, [])]
        return "written_unverified", paths
    if source.startswith("Core/Sources/CoveUIScene/"):
        relative = source.removeprefix("Core/Sources/CoveUIScene/").removesuffix(".swift")
        target = SCENE_GROUPS.get(relative, "/".join(snake(p) for p in relative.split("/")) + ".rs")
        return "written_unverified", ["Core/rust/scene/" + target]
    if source.startswith("PlatformCore/Sources/"):
        return "written_unverified", ["PlatformCore/rust/resources.rs" if path.stem == "PackageResourceBundle" else "PlatformCore/rust/lib.rs"]
    if source.startswith("Text/Sources/CoveUIText/"):
        return "written_unverified", ["Text/rust/" + ("lib.rs" if path.stem == "Exports" else snake(path.stem) + ".rs")]
    if source.startswith("Imaging/Sources/CoveUIImaging/"):
        return "written_unverified", ["Imaging/rust/" + ("lib.rs" if path.stem == "Exports" else snake(path.stem) + ".rs")]
    return "pending", []

def inventory() -> dict:
    tracked = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT).decode().split("\0")
    entries = []
    for source in sorted(tracked):
        if not (source.endswith(".swift") or source.endswith(".swift.in")) or source.startswith(("third-party/", "Native/third-party/")):
            continue
        state, targets = mapped_paths(source)
        existing = [target for target in targets if (ROOT / target).is_file()]
        if targets and len(existing) != len(targets):
            state = "partial" if existing else "pending"
        if not targets:
            state = "pending"
        data = (ROOT / source).read_bytes()
        entries.append({"source": source, "source_sha256": hashlib.sha256(data).hexdigest(),
                        "source_lines": len(data.splitlines()), "status": state, "rust": targets})
    return {"schema": 1, "verification": "not_run_until_full_translation", "entries": entries}

def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="write Docs/rust-migration-files.json")
    args = parser.parse_args()
    data = inventory()
    if args.write:
        OUTPUT.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n")
    else:
        print(json.dumps(data, ensure_ascii=False, indent=2))

if __name__ == "__main__":
    main()
