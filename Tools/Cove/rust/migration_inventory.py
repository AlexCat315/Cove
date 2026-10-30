#!/usr/bin/env python3
"""Reconcile Docs/rust-migration-files.json against the Rust tree on disk.

The inventory was hand-maintained and drifted: it recorded `pending` for files
that now have implementations, and left `rust: []` for modules that were later
written. This script derives the Rust target for every Swift production source
from the files actually on disk, so the JSON and Markdown reports describe the
repository rather than an earlier snapshot.

Matching rules, in order:
1. An existing `rust` entry is kept when the file exists.
2. Otherwise a `.rs` file with the same snake_case stem is preferred, nearest
   directory first (same module, then same crate, then the whole tree).
3. A directory-named module (e.g. `SceneInspector.swift` -> `inspector.rs` or
   `scene_inspector.rs`) is accepted through the same stem comparison.

Status is derived, never guessed:
- `written_unverified` when at least one target exists on disk
- `pending` with `planned: true` only when no target could be found
"""

from __future__ import annotations

import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
JSON_PATH = os.path.join(ROOT, "Docs", "rust-migration-files.json")

EXCLUDED_DIRS = {
    ".git", ".build", ".swiftpm", "target", "third-party", "vendor",
    "node_modules", "build", "dist", ".cache", ".compare", ".workbuddy-ai",
    "Artifacts", "Website", "skills", "agent",
}


def snake(name: str) -> str:
    """`SceneInspector` / `sceneInspector` -> `scene_inspector`."""
    name = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", name)
    name = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", name)
    return name.lower().replace("-", "_")


def rust_files() -> list[str]:
    found: list[str] = []
    for dirpath, dirnames, filenames in os.walk(ROOT):
        dirnames[:] = [d for d in dirnames if d not in EXCLUDED_DIRS]
        for filename in filenames:
            if filename.endswith(".rs"):
                found.append(os.path.relpath(os.path.join(dirpath, filename), ROOT))
    return sorted(found)


def crate_root(path: str) -> str:
    """Longest ancestor directory that holds a Cargo.toml."""
    current = os.path.dirname(path)
    best = ""
    while current and current != ROOT:
        if os.path.isfile(os.path.join(ROOT, current, "Cargo.toml")):
            best = current
        parent = os.path.dirname(current)
        if parent == current:
            break
        current = parent
    return best


def score(swift_source: str, candidate: str, stem: str) -> int:
    """Higher is a better match. `None`-equivalent means 'not a match'."""
    candidate_stem = os.path.basename(candidate)[:-3]
    if candidate_stem == "mod" or candidate_stem == "lib":
        return 0
    candidate_snake = snake(candidate_stem)
    exact = candidate_stem == stem or candidate_snake == stem
    if not exact:
        # Swift files often carry a module prefix that the Rust module drops:
        # `DX12Backend.swift` -> `dx12/backend.rs`, `MetalDevice.swift` ->
        # `metal/device.rs`, `CoveUIWorkspaceModel.swift` -> `workspace/model.rs`.
        # A suffix match on a snake_case word boundary covers that shape.
        # A prefixed Swift name (`DX12Backend`) may drop its prefix in Rust
        # (`backend`), but the *whole* remaining suffix must be the candidate
        # stem — otherwise `Visibility` would match `animated_visibility`.
        suffix = stem.removeprefix("_")
        exact = suffix == candidate_snake or (
            suffix.endswith("_" + candidate_snake) and len(candidate_snake) >= 4
        )
    if not exact:
        return 0

    swift_parts = swift_source.split(os.sep)
    candidate_parts = candidate.split(os.sep)
    path = 1
    swift_crate = crate_root(swift_source)
    if swift_crate and crate_root(candidate) == swift_crate:
        path += 4
    else:
        # A candidate in a different crate is only plausible when the *stem*
        # match is on a shared, generic word (`Visibility`, `Support`,
        # `Options`). Those collide across subsystems, so require the Swift
        # file's own directory name to appear in the candidate's path.
        swift_parent = swift_parts[-2].lower() if len(swift_parts) > 1 else ""
        candidate_lower = candidate.lower()
        if not swift_parent or swift_parent not in candidate_lower:
            return 0
    # Prefer the same relative subdirectory shape.
    if swift_parts[:2] == candidate_parts[:2]:
        path += 2
    if len(candidate_parts) > 1 and swift_parts[-2].lower() in (
        candidate_parts[-2].lower(),
        snake(candidate_parts[-2].lower()),
    ):
        path += 2
    # A `mod.rs` next to the file, or a file inside a same-named directory, is
    # the conventional Rust layout for a Swift type in its own file.
    if os.path.basename(candidate) != "mod.rs":
        path += 1
    return path


def derive(entries: list[dict], rust: list[str]) -> None:
    declared = declared_modules()
    for entry in entries:
        if entry["kind"] != "production":
            continue
        # `verified_by: "operator"` marks a row a human checked by symbol. The
        # matcher cannot infer every mapping (Swift `Skill.swift` is Rust
        # `AgentSkill`), so those decisions are authoritative and never re-derived.
        if entry.get("verified_by") == "operator":
            continue
        if entry.get("status") == "pending" and not entry.get("rust"):
            # An explicitly cleared entry is an operator correction (the previous
            # target was a wrong mapping); leave it pending.
            continue
        existing = [
            p
            for p in entry.get("rust", [])
            if os.path.exists(os.path.join(ROOT, p)) and is_reachable(p, declared)
        ]
        if existing:
            entry["rust"] = existing
            entry["status"] = "written_unverified"
            entry["planned"] = False
            continue

        source = entry["source"]
        stem = snake(os.path.splitext(os.path.basename(source))[0])
        ranked = sorted(
            ((score(source, candidate, stem), candidate) for candidate in rust),
            key=lambda pair: (-pair[0], pair[1]),
        )
        best = [candidate for value, candidate in ranked if value > 0][:1]
        if best:
            entry["rust"] = best
            entry["status"] = "written_unverified"
            entry["planned"] = False
        else:
            # A stale target list must not survive: it is what made the
            # `Visibility.swift` collision look like coverage.
            entry["rust"] = []
            entry["status"] = "pending"
            entry["planned"] = True


# Type names that are ordinary words and therefore collide across subsystems.
GENERIC_TYPE_NAMES = {
    "visibility", "support", "options", "values", "style", "layout", "content",
    "state", "selection", "presentation", "resolution", "split", "tree", "model",
    "handler", "service", "store", "reader", "host", "result", "item", "row",
}

# `impl View for Name` and `impl<T: ...> View for Name<T>` are both
# renderable declarations. Matching only the non-generic form silently
# under-reports coverage — a bug found twice (ButtonGroup, TableColumn).
_RENDERABLE_CACHE: dict[str, str] = {}


def renderable_index() -> dict[str, list[str]]:
    """Public type name -> Rust files that give it a `View`/`PrimitiveView` impl."""
    index: dict[str, list[str]] = {}
    pattern = re.compile(
        r"impl(?:<[^>]*>)?\s+(?:crate::)?(?:View|PrimitiveView)\s+for\s+([A-Za-z0-9_]+)"
    )
    macro = re.compile(r"impl_primitive_view!\(\s*([A-Za-z0-9_]+)")
    for path in rust_files():
        try:
            text = open(os.path.join(ROOT, path), encoding="utf-8", errors="ignore").read()
        except OSError:
            continue
        for match in pattern.finditer(text):
            index.setdefault(match.group(1), []).append(path)
        for match in macro.finditer(text):
            index.setdefault(match.group(1), []).append(path)
    return index


_HEADER_CACHE: dict[str, str] = {}


def source_header(path: str) -> str:
    """The leading `//!` doc block of a Rust file, cached."""
    cached = _HEADER_CACHE.get(path)
    if cached is not None:
        return cached
    lines: list[str] = []
    try:
        with open(os.path.join(ROOT, path), encoding="utf-8", errors="ignore") as handle:
            for line in handle:
                stripped = line.strip()
                if stripped.startswith("//!") or stripped.startswith("//"):
                    lines.append(stripped)
                elif stripped:
                    break
    except OSError:
        pass
    header = "\n".join(lines)
    _HEADER_CACHE[path] = header
    return header


def type_index() -> dict[str, list[str]]:
    """Public type/function name -> Rust files that define it.

    Several Swift files collapse into one Rust module (for example
    `Sources/CoveUIWebView/{WebView,WebViewHandle,WebViewState}.swift` all live
    in `Sources/rust/webview/view.rs`). A shared target is still coverage, so a
    pending file whose primary type exists anywhere in Rust counts as written.
    """
    index: dict[str, list[str]] = {}
    for path in rust_files():
        try:
            text = open(os.path.join(ROOT, path), encoding="utf-8", errors="ignore").read()
        except OSError:
            continue
        for match in re.finditer(
            r"^\s*pub (?:struct|enum|trait|type|const|fn) ([A-Za-z0-9_]+)", text, re.M
        ):
            index.setdefault(match.group(1), []).append(path)
    return index


STATUS_LABEL = {
    "pending": "未开始",
    "partial": "进行中（有缺项）",
    "written_unverified": "已写入（未验证）",
    "manifest_written": "Cargo 清单已建立",
}


def write_markdown(document: dict) -> None:
    """Regenerate the per-file table so it cannot drift from the JSON."""
    by_kind: dict[str, list[dict]] = {}
    for entry in document["entries"]:
        by_kind.setdefault(entry["kind"], []).append(entry)

    lines = [
        "# Rust 迁移逐文件映射表",
        "",
        "由 `Docs/rust-migration-files.json` 生成，状态必须与该 JSON 一致。"
        "`（计划）` 表示 Rust 目标路径尚未创建，只是登记的目标位置。",
        "",
        "## 口径",
        "",
        "- 生产源码：排除测试目录、SwiftPM 清单、CLI 项目模板与所有构建/依赖产物。",
        "- 排除路径：`**/.build/**`、`third-party/**`、`vendor/**`、`target/**`、"
        "`dist/**`、`Artifacts/**`、`build/**`、`node_modules/**`、`.git/**`。",
        "- `Tests/**` 与 `**/Tests/**` 归为测试源码（kind=`test`）；"
        "`Package.swift` 归为清单（kind=`manifest`）；`**/Templates/**` 归为模板（kind=`template`）。",
        "- 状态：`未开始` / `进行中（有缺项）` / `已写入（未验证）` / `Cargo 清单已建立`。"
        "`已写入` 不代表通过编译、行为对照或平台验收。",
        "",
        "## 汇总",
        "",
        "| 范围 | 文件数 | 已写入 | 进行中 | 未开始 |",
        "| --- | ---: | ---: | ---: | ---: |",
    ]
    labels = [
        ("production", "生产源码"),
        ("test", "测试源码"),
        ("manifest", "SwiftPM 清单"),
        ("template", "项目模板"),
    ]
    total = 0
    for kind, label in labels:
        entries = by_kind.get(kind, [])
        total += len(entries)
        written = sum(1 for e in entries if e["status"] in ("written_unverified", "manifest_written"))
        partial = sum(1 for e in entries if e["status"] == "partial")
        pending = sum(1 for e in entries if e["status"] == "pending")
        lines.append(f"| {label} | {len(entries)} | {written} | {partial} | {pending} |")
    lines.append(f"| 合计 | {total} | — | — | — |")
    lines.append("")

    modules: dict[str, list[dict]] = {}
    for entry in sorted(by_kind.get("production", []), key=lambda e: (e["module"], e["source"])):
        modules.setdefault(entry["module"], []).append(entry)

    lines += ["## 生产源码逐文件映射", ""]
    for module in sorted(modules, key=lambda m: -len(modules[m])):
        entries = modules[module]
        written = sum(1 for e in entries if e["status"] == "written_unverified")
        partial = sum(1 for e in entries if e["status"] == "partial")
        pending = sum(1 for e in entries if e["status"] == "pending")
        lines += [
            f"### {module}",
            "",
            f"共 {len(entries)} 个文件：已写入 {written}，进行中 {partial}，未开始 {pending}。",
            "",
            "| 源文件 | 行 | 状态 | Rust/C 目标 | 依赖或阻塞 |",
            "| --- | ---: | --- | --- | --- |",
        ]
        for entry in entries:
            targets = "<br>".join(f"`{path}`" for path in entry["rust"]) or "—"
            if entry["status"] == "pending":
                note = "等待实现"
            else:
                note = "未编译、未行为对照"
            lines.append(
                f"| `{entry['source']}` | {entry['source_lines']} | "
                f"{STATUS_LABEL.get(entry['status'], entry['status'])} | {targets} | {note} |"
            )
        lines.append("")

    with open(os.path.join(ROOT, "Docs", "rust-migration-files.md"), "w", encoding="utf-8") as handle:
        handle.write("\n".join(lines))
        handle.write("\n")


def manifest_paths() -> list[str]:
    """Every Cargo.toml in the tree, relative to the repo root."""
    found: list[str] = []
    for dirpath, dirnames, filenames in os.walk(ROOT):
        dirnames[:] = [d for d in dirnames if d not in EXCLUDED_DIRS]
        for filename in filenames:
            if filename == "Cargo.toml":
                found.append(os.path.relpath(os.path.join(dirpath, filename), ROOT))
    return sorted(found)


def collapse_test_name(name: str) -> str:
    """`ZStackTests` / `z_stack_tests` / `zstack` all collapse to `zstack`."""
    lowered = re.sub(r"[^a-z0-9]", "", name.lower())
    return re.sub(r"tests$", "", lowered)


def test_target_names() -> list[str]:
    """Names of every declared Cargo test target.

    Reads `[[test]]` tables out of the crate manifests: the `name` field is what
    a translated Swift suite is registered under, and the `path` proves the file
    exists on disk.
    """
    names: list[str] = []
    for path in manifest_paths():
        try:
            text = open(os.path.join(ROOT, path), encoding="utf-8").read()
        except OSError:
            continue
        for block in text.split("[[test]]")[1:]:
            name = re.search(r'^\s*name\s*=\s*"([^"]+)"', block, re.M)
            target = re.search(r'^\s*path\s*=\s*"([^"]+)"', block, re.M)
            if not name:
                continue
            if target and not os.path.exists(
                os.path.join(ROOT, os.path.dirname(path), target.group(1))
            ):
                continue
            names.append(name.group(1))
    return names


_DECLARED_CACHE: dict[str, set[str]] = {}


def declared_modules() -> set[str]:
    """Every module name declared by any `mod` statement in the tree.

    A `.rs` file whose stem is never declared is dead code: the compiler never
    sees it, so it cannot be "written" in any meaningful sense. This project has
    produced that class of bug repeatedly (System 16, Logging 15, Tools 20).
    """
    names: set[str] = set()
    for dirpath, dirnames, filenames in os.walk(ROOT):
        dirnames[:] = [d for d in dirnames if d not in EXCLUDED_DIRS]
        for filename in filenames:
            if not filename.endswith(".rs"):
                continue
            try:
                text = open(os.path.join(dirpath, filename), encoding="utf-8", errors="ignore").read()
            except OSError:
                continue
            for match in re.finditer(
                r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z0-9_]+)\s*;", text, re.M
            ):
                names.add(match.group(1))
    return names


def is_reachable(path: str, declared: set[str]) -> bool:
    """Whether a Rust file is named by a `mod` statement in its own directory.

    The declaration must live in the parent module: `agent/skills.rs` is reached
    by `mod skills;` inside `agent/mod.rs`, not by a same-named `mod` anywhere in
    the tree. Checking the directory keeps the answer honest — a file declared
    only in an unrelated crate's sibling is still dead.
    """
    stem = os.path.basename(path)[:-3]
    if stem in ("lib", "main", "build", "mod"):
        return True
    directory = os.path.dirname(path)
    for candidate in (
        os.path.join(directory, "mod.rs"),
        os.path.join(directory, "lib.rs"),
        # A binary crate's root is `main.rs`; `Tools/Cove` is one.
        os.path.join(directory, "main.rs"),
        os.path.join(os.path.dirname(directory), os.path.basename(directory) + ".rs"),
    ):
        full = os.path.join(ROOT, candidate)
        if not os.path.exists(full):
            continue
        try:
            text = open(full, encoding="utf-8", errors="ignore").read()
        except OSError:
            continue
        if re.search(
            r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+" + re.escape(stem) + r"\s*;",
            text,
            re.M,
        ):
            return True
    return False


def main() -> int:
    with open(JSON_PATH, encoding="utf-8") as handle:
        document = json.load(handle)

    rust = rust_files()
    derive(document["entries"], rust)

    # Test sources map onto `[[test]]` targets rather than onto a single file:
    # a Swift test suite becomes a Rust integration test whose name is the
    # snake_case stem of the Swift file.
    declared_tests = test_target_names()

    # Second pass: a Swift file whose primary type is already defined by some
    # Rust module is covered even when the file naming differs.
    types = type_index()
    for entry in document["entries"]:
        if entry["status"] != "pending":
            continue
        stem = os.path.splitext(os.path.basename(entry["source"]))[0]
        if entry["kind"] == "test":
            # Swift suites are named `<Name>Tests`; the Rust target drops the
            # suffix and sometimes the word boundaries (`ZStackTests` ->
            # `zstack`). Compare on collapsed alphanumerics with the suffix
            # removed, so both spellings match.
            matches = [target for target in declared_tests
                       if collapse_test_name(target) == collapse_test_name(stem)]
            if matches:
                entry["rust"] = matches
                entry["status"] = "written_unverified"
                entry["planned"] = False
            continue
        if entry["kind"] != "production":
            continue
        # Common English words are used as type names by unrelated subsystems
        # (`Visibility` is a GPU buffer and a table column plan), so a bare name
        # hit is not evidence. Only distinctive names are matched this way.
        if types.get(stem) and stem.lower() not in GENERIC_TYPE_NAMES:
            usable = [
                path
                for path in types[stem]
                if path.endswith("mod.rs") is False
                and os.path.basename(path)[:-3] not in ("lib", "main", "build")
            ]
            if usable:
                entry["rust"] = sorted(set(usable))[:3]
                entry["status"] = "written_unverified"
                entry["planned"] = False
                continue
        # A Swift file whose primary type is defined under a different Rust name
        # (Swift `Skill.swift` -> Rust `AgentSkill`) is still covered. Match on the
        # *declared type prefix*: the Swift stem must be a suffix of a public type
        # name in the Swift file's own crate.
        collapsed_stem = re.sub(r"[^a-z0-9]", "", stem.lower())
        declared_types = {
            name
            for name in types
            if name.lower().endswith(collapsed_stem) and name.lower() != collapsed_stem
        }
        if declared_types:
            swift_crate = crate_root(entry["source"])
            usable = sorted({
                path
                for name in declared_types
                for path in types[name]
                if swift_crate and crate_root(path) == swift_crate
            })
            if usable:
                entry["rust"] = usable[:3]
                entry["status"] = "written_unverified"
                entry["planned"] = False
                continue

        # Consolidated modules document the Swift files they replace in their
        # header (e.g. "Translation of .../ResourcesSupport/Bundle.swift and
        # .../Lookup.swift"), so a filename mention there is coverage evidence.
        # Match the full Swift basename, not the bare stem: `Visibility.swift`
        # must not be "found" inside a header that says `AnimatedVisibility.swift`.
        # Require a path separator before the basename: `Visibility.swift` is a
        # substring of `AnimatedVisibility.swift`, and a bare `in` test would
        # credit the wrong module.
        basename = os.path.basename(entry["source"])
        pattern = re.compile(r"(?<![A-Za-z0-9_])" + re.escape(basename))
        mentioned = [path for path in rust if pattern.search(source_header(path))]
        if mentioned:
            entry["rust"] = sorted(set(mentioned))[:3]
            entry["status"] = "written_unverified"
            entry["planned"] = False
            continue
        if False:
            entry["rust"] = sorted(set(types[stem]))[:3]
            entry["status"] = "written_unverified"
            entry["planned"] = False

    # Final pass: a claim backed only by unreachable files is not coverage.
    declared = declared_modules()
    demoted = 0
    for entry in document["entries"]:
        if entry["status"] != "written_unverified" or entry["kind"] != "production":
            continue
        reachable = [p for p in entry.get("rust", []) if is_reachable(p, declared)]
        if entry.get("rust") and not reachable:
            entry["status"] = "pending"
            entry["planned"] = True
            entry["rust"] = []
            demoted += 1

    with open(JSON_PATH, "w", encoding="utf-8") as handle:
        json.dump(document, handle, indent=2, ensure_ascii=False)
        handle.write("\n")

    write_markdown(document)
    production = [e for e in document["entries"] if e["kind"] == "production"]
    pending = [e for e in production if e["status"] == "pending"]
    print(f"production entries: {len(production)}")
    print(f"written (some file on disk): {len(production) - len(pending)}")
    print(f"pending: {len(pending)}")
    print(f"demoted (target files are unreachable dead code): {demoted}")
    for entry in pending:
        print(f"  MISSING {entry['module']:12s} {entry['source']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
