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
PARTIAL_SOURCES = {
    # The core interaction/style host and loading indicator are present; the
    # transient tooltip overlay and Escape-to-dismiss flow remain to be ported.
    "Compose/Sources/CoveUICompose/Primitives/Controls/Button.swift",
    # Link is wired through Button but still lacks tooltip presentation and
    # interaction animation; Tag is wired, but its remove tooltip needs Button's
    # unported tooltip presentation.
    "Compose/Sources/CoveUICompose/Primitives/Controls/Link.swift",
    "Compose/Sources/CoveUICompose/Primitives/Controls/Tag.swift",
    # Keyboard shortcuts and option values are ported; the Select trigger and
    # popup/menu interaction still need their dedicated Rust implementation.
    "Compose/Sources/CoveUICompose/Primitives/Controls/Fields/Select.swift",
    # Visibility policy is implemented; the picker still needs the adaptive
    # menu and overlay host.
    "Compose/Sources/CoveUICompose/Primitives/Columns/Picker.swift",
    # Plain rendering/layout is present; tooltip, interactive truncation state,
    # and the selectable-label bridge still need to be ported.
    "Compose/Sources/CoveUICompose/Primitives/Text/Text.swift",
}

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
    if source.startswith("Runtime/Sources/CoveUIRuntime/"):
        targets = {"CoveUIRuntime": ["lib.rs"], "Exports": ["lib.rs"], "DisplayDriver": ["ios/mod.rs", "lifecycle.m"],
            "NativeAccessibility": ["native_accessibility.rs", "accessibility.m"]}
        return "written_unverified", ["Runtime/rust/" + target for target in targets.get(path.stem, [snake(path.stem) + ".rs"])]
    if source.startswith("Compose/Composition/Sources/CoveUIComposeCore/"):
        relative = source.removeprefix("Compose/Composition/Sources/CoveUIComposeCore/").removesuffix(".swift")
        targets = {
            "Exports": "lib.rs",
            "Box": "box_view.rs",
            "ModifiedContent+Materialise": "modified_content_materialise.rs",
            "Layout/Layout": "layout/mod.rs",
            "Layout/LayoutValue": "layout/value.rs",
            "TrackStateReads": "track_state_reads.rs",
        }
        target = targets.get(relative, "/".join(snake(part) for part in relative.split("/")) + ".rs")
        return "written_unverified", ["Compose/Composition/rust/" + target]
    if source.startswith("Compose/Composition/Sources/CoveUIReadMacros/"):
        return "written_unverified", ["Compose/CompositionMacros/rust/lib.rs"]
    if source.startswith("Compose/Sources/CoveUICompose/"):
        relative = source.removeprefix("Compose/Sources/CoveUICompose/").removesuffix(".swift")
        if relative == "Primitives/Controls/Button/Chrome":
            return "written_unverified", ["Compose/rust/theme/styles/buttons/builtin_button_chrome.rs"]
        if relative == "Modifiers/Layout/LetterSpacingModifier":
            return "written_unverified", ["Compose/rust/modifiers/style/style_modifiers.rs"]
        interaction_modifiers = {
            "Modifiers/Interaction/AccessibilityModifier",
            "Modifiers/Interaction/CursorModifier",
            "Modifiers/Interaction/FocusRequestModifier",
            "Modifiers/Interaction/HitTestingModifier",
            "Modifiers/Interaction/HoverModifier",
        }
        if relative in interaction_modifiers:
            return "written_unverified", ["Compose/rust/modifiers/interaction/mod.rs"]
        layout_modifiers = {
            "Modifiers/Layout/AbsolutePositionModifier",
            "Modifiers/Layout/FlexModifier",
            "Modifiers/Layout/FrameModifier",
            "Modifiers/Layout/PaddingModifier",
            "Modifiers/Layout/PaddingSugar",
        }
        if relative in layout_modifiers:
            return "written_unverified", ["Compose/rust/modifiers/layout/mod.rs"]
        if relative == "Primitives/Feedback/StatusTone":
            return "written_unverified", ["Compose/rust/primitives/feedback/status_tone.rs"]
        if relative == "Modifiers/Layout/FrameCaptureModifier":
            return "written_unverified", ["Compose/rust/modifiers/layout/frame_capture.rs"]
        if relative == "Modifiers/Style/SurfaceFinishModifier":
            return "written_unverified", ["Compose/rust/modifiers/style/surface.rs"]
        if relative == "Animation/AnimationValueModifier":
            return "written_unverified", ["Compose/rust/modifiers/animation_value.rs"]
        if relative == "Animation/EdgeInsets+Interpolatable":
            return "written_unverified", ["Compose/rust/modifiers/layout/mod.rs"]
        if relative == "Animation/WithAnimation":
            return "written_unverified", ["Compose/rust/lib.rs", "Core/rust/scene/animation/context.rs"]
        if relative == "CoveUIComposeResourceBundle":
            return "written_unverified", ["Compose/rust/resource_bundle.rs", "PlatformCore/rust/resources.rs"]
        if relative == "Primitives/Columns/Priority":
            return "written_unverified", ["Compose/rust/primitives/columns/mod.rs"]
        if relative == "Primitives/Columns/Picker":
            return "partial", ["Compose/rust/primitives/columns/picker.rs"]
        target = "lib.rs" if relative in {"Exports", "CoveUICompose"} else "/".join(snake(part) for part in relative.split("/")) + ".rs"
        return "written_unverified", ["Compose/rust/" + target]
    if source.startswith("Graphics/Sources/CoveUIGraphics/"):
        targets = {"Exports": "mod", "UIAssetResidency": "asset_residency", "NativeUIViewport": "viewport",
            "DrawGeometry": "draw_geometry", "Resources": "resources", "NativeUIShaderResources": "shader_resources",
            "NativeUIRasterWorkspace": "raster_workspace", "NativeUITextureStore": "texture_store",
            "NativeUIBackdropPass": "backdrop_pass", "NativeDrawListRenderer": "draw_list_renderer"}
        return "written_unverified", ["Graphics/rust/ui/" + targets.get(path.stem, snake(path.stem)) + ".rs"]
    if source == "Graphics/Sources/NativeRHI/Resources/ShaderArtifactLoading.swift":
        return "written_unverified", ["Graphics/rust/rhi/shader_artifact_loading.rs"]
    if source.startswith("Shell/Sources/PlatformShell/"):
        targets = {
            "PlatformShell": ["platform_shell.rs", "sdl3_shell.rs", "apple.m"],
            "SDL3Shell": ["sdl3_shell.rs", "native.c", "apple.m"],
            "Signal": ["display_refresh/signal.rs"],
            "WindowDisplayLink": ["display_refresh/mod.rs", "apple.m"],
            "CoreVideo": ["display_refresh/mod.rs", "apple.m"],
            "MacDisplayLink": ["display_refresh/mod.rs", "apple.m"],
            "AppKitInput": ["scroll/mod.rs", "apple.m"],
            "SDL": ["diagnostics/mod.rs"],
        }
        return "written_unverified", ["Shell/rust/" + target for target in targets.get(path.stem, [snake(path.stem) + ".rs"])]
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
        elif source in PARTIAL_SOURCES and state == "written_unverified":
            state = "partial"
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
