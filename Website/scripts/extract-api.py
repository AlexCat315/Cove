#!/usr/bin/env python3
"""
Extract CoveUI's public API surface from the Swift sources into JSON that the
website renders as an Apple-style reference.

Nothing here is hand-written: every declaration, abstract and availability
line is copied out of the source tree, together with the file and line it came
from. Run from the Website directory:

    python3 scripts/extract-api.py
"""

from __future__ import annotations

import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]  # Cove/
OUT_DIR = Path(__file__).resolve().parents[1] / "src" / "content" / "api"

# module label -> source roots (relative to the repository root)
MODULES: dict[str, list[str]] = {
    "CoveUIComposeCore": ["Compose/Composition/Sources/CoveUIComposeCore"],
    "CoveUICompose": ["Compose/Sources/CoveUICompose"],
    "CoveUIGraphics": ["Graphics/Sources"],
    "CoveUIText": ["Text/Sources"],
    "CoveUICore": ["Core/Sources"],
    "CoveUIShell": ["Shell/Sources"],
    "CoveUIRuntime": ["Runtime/Sources"],
    "CoveUI": ["Hosts/Desktop/Sources/CoveUI"],
    "CoveUITerminal": ["Terminal/Sources"],
}

SKIP_DIRS = {".build", "node_modules", "Tests", "third-party", "vendor", "dist", "Resources"}

# Directory prefix -> category id. Longest matching prefix wins.
CATEGORY_RULES: list[tuple[str, str]] = [
    ("Compose/Composition/Sources/CoveUIComposeCore", "core"),
    ("Compose/Sources/CoveUICompose/Modifiers/Layout", "layout"),
    ("Compose/Sources/CoveUICompose/Modifiers/Style", "style"),
    ("Compose/Sources/CoveUICompose/Modifiers/Interaction", "interaction"),
    ("Compose/Sources/CoveUICompose/Primitives/Text", "text"),
    ("Compose/Sources/CoveUICompose/Primitives/Layout", "layout"),
    ("Compose/Sources/CoveUICompose/Primitives/Controls", "controls"),
    ("Compose/Sources/CoveUICompose/Primitives/Navigation", "navigation"),
    ("Compose/Sources/CoveUICompose/Primitives/Overlays", "overlays"),
    ("Compose/Sources/CoveUICompose/Primitives/Data", "data"),
    ("Compose/Sources/CoveUICompose/Primitives/Media", "media"),
    ("Compose/Sources/CoveUICompose/Primitives/Feedback", "feedback"),
    ("Compose/Sources/CoveUICompose/Primitives/Interaction", "interaction"),
    ("Compose/Sources/CoveUICompose/Primitives/Columns", "controls"),
    ("Compose/Sources/CoveUICompose/Primitives", "views"),
    ("Compose/Sources/CoveUICompose/Animation", "animation"),
    ("Compose/Sources/CoveUICompose/Theme", "theme"),
    ("Compose/Sources/CoveUICompose/Scrolling", "layout"),
    ("Compose/Sources/CoveUICompose/AppStorage.swift", "data"),
    ("Compose/Sources/CoveUICompose/Observed.swift", "data"),
    ("Compose/Sources/CoveUICompose/I18n", "text"),
    ("Compose/Sources/CoveUICompose/Foundation", "style"),
    ("Compose/Sources/CoveUICompose/Pet", "views"),
    ("Compose/Sources/CoveUICompose/Rendering", "core"),
    ("Compose/Sources/CoveUICompose/Core", "core"),
    ("Compose/Sources/CoveUICompose", "views"),
    ("Graphics/Sources", "graphics"),
    ("Text/Sources", "text"),
    ("Core/Sources", "core"),
    ("Shell/Sources", "shell"),
    ("Runtime/Sources", "runtime"),
    ("Hosts/Desktop/Sources/CoveUI/Application", "app"),
    ("Hosts/Desktop/Sources/CoveUI/Window", "shell"),
    ("Hosts/Desktop/Sources/CoveUI/Display", "shell"),
    ("Hosts/Desktop/Sources/CoveUI/Diagnostics", "devtools"),
    ("Hosts/Desktop/Sources/CoveUI/Rendering", "graphics"),
    ("Hosts/Desktop/Sources/CoveUI", "app"),
    ("Terminal/Sources", "terminal"),
]

CATEGORY_TITLES = {
    "core": "核心",
    "views": "示例视图",
    "text": "文本",
    "layout": "布局",
    "controls": "控件",
    "navigation": "导航",
    "overlays": "浮层与弹窗",
    "data": "数据展示",
    "media": "图像与媒体",
    "feedback": "状态与反馈",
    "animation": "动画",
    "theme": "主题",
    "style": "外观与样式",
    "interaction": "交互",
    "graphics": "图形",
    "shell": "外壳与窗口",
    "runtime": "运行时",
    "app": "应用与生命周期",
    "devtools": "开发工具",
    "terminal": "终端",
}

CATEGORY_ORDER = list(CATEGORY_TITLES)

TYPE_KINDS = {"struct", "enum", "protocol", "class", "actor"}


@dataclass
class Member:
    name: str
    kind: str
    declaration: str
    abstract: str
    discussion: list[str]

    def to_json(self) -> dict:
        return {
            "name": self.name,
            "kind": self.kind,
            "declaration": self.declaration,
            "abstract": self.abstract,
            "discussion": self.discussion,
        }


@dataclass
class Symbol:
    name: str
    kind: str
    declaration: str
    module: str
    category: str
    file: str
    line: int
    abstract: str = ""
    discussion: list[str] = field(default_factory=list)
    availability: list[str] = field(default_factory=list)
    conforms: list[str] = field(default_factory=list)
    members: list[Member] = field(default_factory=list)
    is_modifier: bool = False
    owner: str = ""

    def to_json(self, symbol_id: str) -> dict:
        return {
            "id": symbol_id,
            "name": self.name,
            "kind": "modifier" if self.is_modifier else self.kind,
            "declaration": self.declaration,
            "abstract": self.abstract,
            "discussion": self.discussion,
            "availability": self.availability,
            "conforms": self.conforms,
            "members": [m.to_json() for m in self.members if not m.name.startswith("_")],
            "module": self.module,
            "category": self.category,
            "file": self.file,
            "line": self.line,
            "owner": self.owner,
        }


def camel_to_kebab(name: str) -> str:
    name = name.strip("_")
    out = re.sub(r"(?<=[a-z0-9])([A-Z])", r"-\1", name)
    out = re.sub(r"(?<=[A-Z])([A-Z][a-z])", r"-\1", out)
    return re.sub(r"[^A-Za-z0-9]+", "-", out).strip("-").lower() or "symbol"


def strip_code(line: str) -> str:
    """Remove string literals and line comments so brace counting is stable."""
    out = []
    i = 0
    in_string = False
    while i < len(line):
        ch = line[i]
        if in_string:
            if ch == "\\":
                i += 2
                continue
            if ch == '"':
                in_string = False
            i += 1
            continue
        if ch == '"':
            in_string = True
            i += 1
            continue
        if ch == "/" and i + 1 < len(line) and line[i + 1] == "/":
            break
        out.append(ch)
        i += 1
    return "".join(out)


def doc_block(lines: list[str], decl_index: int) -> tuple[str, list[str], list[str]]:
    """Collect the doc comment and attributes immediately above a declaration."""
    i = decl_index - 1
    attributes: list[str] = []
    while i >= 0:
        stripped = lines[i].strip()
        if stripped.startswith("@") or stripped.startswith("#if") or stripped.startswith("#endif"):
            attributes.insert(0, stripped)
            i -= 1
            continue
        break

    doc: list[str] = []
    while i >= 0:
        stripped = lines[i].strip()
        if stripped.startswith("///"):
            doc.insert(0, stripped[3:][1:] if stripped[3:].startswith(" ") else stripped[3:])
            i -= 1
            continue
        break

    availability: list[str] = []
    for attr in attributes:
        m = re.match(r"@available\((.+)\)", attr)
        if m:
            availability.append(m.group(1).replace(", ", " "))

    text = "\n".join(doc).strip()
    if not text:
        return "", [], availability

    # First paragraph (possibly a fenced snippet) becomes the abstract.
    paragraphs = re.split(r"\n\s*\n", text)
    abstract_raw = paragraphs[0].strip()
    if abstract_raw.startswith("```"):
        abstract_raw = ""
    abstract = re.sub(r"\s+", " ", abstract_raw).strip()
    discussion = [p.strip() for p in paragraphs[1:] if p.strip()]
    if abstract_raw:
        discussion = [p.strip() for p in paragraphs[1:] if p.strip()]
    else:
        discussion = [p.strip() for p in paragraphs if p.strip()]
    return abstract, discussion, availability


CONTINUATION = re.compile(r"(,\s*$|\(\s*$|->\s*$|:\s*$|=\s*$|\bwhere\s*$)")


def join_declaration(lines: list[str], start: int) -> tuple[str, int]:
    """Join a declaration's lines until its signature closes."""
    buf: list[str] = []
    depth = 0
    i = start
    while i < len(lines) and i < start + 30:
        stripped = lines[i].strip()
        code = strip_code(stripped)
        if not buf and (stripped.startswith("//") or stripped.startswith("@")):
            i += 1
            continue
        buf.append(stripped)
        depth += code.count("(") - code.count(")")
        if depth <= 0 and ("{" in code or code.rstrip().endswith("}")):
            break
        if depth <= 0 and not CONTINUATION.search(code):
            break
        i += 1
    decl = " ".join(buf)
    decl = decl.split("{")[0] if "{" in decl else decl
    decl = re.sub(r"\s+", " ", decl).strip().rstrip("{").strip()
    return decl, i


def category_for(rel: str) -> str:
    best = ("", "views")
    for prefix, cat in CATEGORY_RULES:
        if (rel == prefix or rel.startswith(prefix + "/")) and len(prefix) > len(best[0]):
            best = (prefix, cat)
    return best[1]


def conformance_of(decl: str) -> list[str]:
    conforms: list[str] = []
    m = re.search(r":\s*([^{]+)$", decl)
    if not m:
        return conforms
    for part in m.group(1).split(","):
        token = part.strip().split(" ")[0]
        if not re.match(r"^_?[A-Z][\w.]*$", token):
            continue
        # `_PrimitiveView` is the internal refinement every primitive uses; its
        # only public guarantee is `View`, so that is what the reference shows.
        conforms.append("View" if token == "_PrimitiveView" else token)
    return list(dict.fromkeys(conforms))


@dataclass
class Container:
    name: str
    is_public: bool
    body_depth: int
    is_extension: bool
    symbol: Symbol | None = None


def parse_file(path: Path, module: str) -> list[Symbol]:
    rel = path.relative_to(ROOT).as_posix()
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except (UnicodeDecodeError, OSError):
        return []

    symbols: list[Symbol] = []
    stack: list[Container] = []
    depth = 0
    i = 0
    n = len(lines)

    while i < n:
        raw = lines[i]
        stripped = raw.strip()
        code = strip_code(raw)
        before = depth

        # --- containers opened at the current depth -------------------------
        ext = re.match(r"^(public\s+)?extension\s+([A-Za-z_][\w.]*)", stripped)
        if ext and "{" in code:
            stack.append(
                Container(
                    name=ext.group(2),
                    is_public=bool(ext.group(1)),
                    body_depth=before,
                    is_extension=True,
                )
            )
            depth += code.count("{") - code.count("}")
            i += 1
            continue

        type_decl = re.match(
            r"^(public|open)\s+(?:final\s+|indirect\s+|nonisolated\s+)*(struct|enum|protocol|class|actor)\s+([A-Za-z_]\w*)",
            stripped,
        )
        if type_decl and (not stack or before == stack[-1].body_depth):
            kind, name = type_decl.group(2), type_decl.group(3)
            decl, end = join_declaration(lines, i)
            abstract, discussion, availability = doc_block(lines, i)
            sym = Symbol(
                name=name,
                kind=kind,
                declaration=decl,
                module=module,
                category=category_for(rel),
                file=rel,
                line=i + 1,
                abstract=abstract,
                discussion=discussion,
                availability=availability,
                conforms=conformance_of(decl),
            )
            symbols.append(sym)
            stack.append(Container(name=name, is_public=True, body_depth=before, is_extension=False, symbol=sym))
            for consumed in lines[i : end + 1]:
                depth += strip_code(consumed).count("{") - strip_code(consumed).count("}")
            i = end + 1
            continue

        # --- top-level public functions and globals --------------------------
        top_fn = re.match(
            r"^(?:@\w+(?:\([^)]*\))?\s+)*(public)\s+(?:func|var|let)\s+([A-Za-z_]\w*)",
            stripped,
        )
        if top_fn and depth == 0 and not stack:
            name = top_fn.group(2)
            decl, end = join_declaration(lines, i)
            abstract, discussion, availability = doc_block(lines, i)
            kind = "func" if re.search(r"\b(public\s+func)\b", decl) else "var"
            symbols.append(
                Symbol(
                    name=name,
                    kind=kind,
                    declaration=decl,
                    module=module,
                    category=category_for(rel),
                    file=rel,
                    line=i + 1,
                    abstract=abstract,
                    discussion=discussion,
                    availability=availability,
                )
            )
            for consumed in lines[i : end + 1]:
                depth += strip_code(consumed).count("{") - strip_code(consumed).count("}")
            i = end + 1
            continue

        # --- members --------------------------------------------------------
        if stack and before == stack[-1].body_depth + 1:
            container = stack[-1]
            explicitly_public = bool(re.match(r"^(?:@\w+(?:\([^)]*\))?\s+)*(public|open)\s", stripped))
            public_ctx = explicitly_public or (container.is_extension and container.is_public)
            if public_ctx:
                member = re.match(
                    r"^(?:@\w+(?:\([^)]*\))?\s+)*(?:public\s+|open\s+)?(?:static\s+|class\s+|mutating\s+|nonisolated\s+)*(func|var|let|init|subscript)\b\s*([A-Za-z_]\w*)?",
                    stripped,
                )
                if member:
                    keyword, name = member.group(1), member.group(2) or ""
                    decl, end = join_declaration(lines, i)
                    abstract, discussion, _availability = doc_block(lines, i)
                    is_modifier = (
                        container.is_extension
                        and container.name.split(".")[-1] == "View"
                        and keyword == "func"
                        and "->" in decl
                    )
                    if keyword == "init":
                        name = container.name.split(".")[-1]
                    if is_modifier:
                        symbols.append(
                            Symbol(
                                name=name,
                                kind="func",
                                declaration=decl,
                                module=module,
                                category=category_for(rel),
                                file=rel,
                                line=i + 1,
                                abstract=abstract,
                                discussion=discussion,
                                is_modifier=True,
                                owner=container.name,
                            )
                        )
                    elif container.symbol is not None:
                        container.symbol.members.append(
                            Member(
                                name=name,
                                kind="init" if keyword == "init" else keyword,
                                declaration=decl,
                                abstract=abstract,
                                discussion=discussion,
                            )
                        )
                    elif container.is_extension:
                        # Members declared in an extension live in a different
                        # file than the type; they are merged in `main`.
                        symbols.append(
                            Symbol(
                                name=name,
                                kind="init" if keyword == "init" else keyword,
                                declaration=decl,
                                module=module,
                                category=category_for(rel),
                                file=rel,
                                line=i + 1,
                                abstract=abstract,
                                discussion=discussion,
                                owner=container.name,
                            )
                        )
                    for consumed in lines[i : end + 1]:
                        depth += strip_code(consumed).count("{") - strip_code(consumed).count("}")
                    i = end + 1
                    continue

        depth += code.count("{") - code.count("}")
        if depth < 0:
            depth = 0
        while stack and depth <= stack[-1].body_depth:
            stack.pop()
        i += 1

    return symbols


def main() -> int:
    all_symbols: list[Symbol] = []
    for module, roots in MODULES.items():
        for root in roots:
            base = ROOT / root
            if not base.exists():
                print(f"skip missing root {root}", file=sys.stderr)
                continue
            for path in sorted(base.rglob("*.swift")):
                rel_parts = path.relative_to(ROOT).parts
                if any(part in SKIP_DIRS for part in rel_parts):
                    continue
                all_symbols.extend(parse_file(path, module))

    # Members declared in extensions are folded into their owning type's page.
    types = [s for s in all_symbols if not s.owner]
    for member in [s for s in all_symbols if s.owner and not s.is_modifier]:
        owner_name = member.owner.split(".")[-1]
        candidates = [
            t
            for t in types
            if t.name == owner_name and not t.is_modifier
        ]
        if not candidates:
            continue
        target = next((t for t in candidates if t.module == member.module), candidates[0])
        if member.name.startswith("_"):
            continue
        if all(m.declaration != member.declaration for m in target.members):
            target.members.append(
                Member(
                    name=member.name,
                    kind=member.kind,
                    declaration=member.declaration,
                    abstract=member.abstract,
                    discussion=member.discussion,
                )
            )
    all_symbols = types + [s for s in all_symbols if s.is_modifier]

    # Merge overloads: same modifier name in the same category is one page with
    # several declarations, exactly like an Apple reference page.
    grouped: dict[tuple, Symbol] = {}
    for sym in all_symbols:
        key = ("modifier", sym.name) if sym.is_modifier else (sym.kind, sym.name, sym.category)
        existing = grouped.get(key)
        if existing is None:
            grouped[key] = sym
            sym.declaration_set = {sym.declaration}  # type: ignore[attr-defined]
            continue
        if sym.declaration not in existing.declaration_set:  # type: ignore[attr-defined]
            existing.declaration_set.add(sym.declaration)  # type: ignore[attr-defined]
            existing.overloads = getattr(existing, "overloads", []) + [sym.declaration]  # type: ignore[attr-defined]
        if not existing.abstract and sym.abstract:
            existing.abstract = sym.abstract
            existing.discussion = sym.discussion
        for member in sym.members:
            if all(m.declaration != member.declaration for m in existing.members):
                existing.members.append(member)

    # Assign ids. First definition keeps the plain slug; later homonyms are
    # qualified with their category so links stay stable and unique.
    used: dict[str, Symbol] = {}
    records: list[tuple[str, dict]] = []
    for sym in grouped.values():
        base = ("modifier-" if sym.is_modifier else "") + camel_to_kebab(sym.name)
        sid = base
        if sid in used:
            sid = f"{base}--{sym.category}"
            n = 2
            while sid in used:
                sid = f"{base}--{sym.category}-{n}"
                n += 1
        used[sid] = sym
        data = sym.to_json(sid)
        overloads = [sym.declaration] + getattr(sym, "overloads", [])
        data["overloads"] = list(dict.fromkeys(overloads))
        records.append((sid, data))

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for old in OUT_DIR.glob("*.json"):
        old.unlink()

    by_category: dict[str, list[dict]] = {}
    for _sid, data in records:
        by_category.setdefault(data["category"], []).append(data)

    manifest = {"categories": [], "symbols": []}
    for cat in CATEGORY_ORDER:
        items = by_category.get(cat)
        if not items:
            continue
        items.sort(key=lambda s: (s["kind"] not in {"struct", "protocol", "enum"}, s["name"].lower()))
        (OUT_DIR / f"{cat}.json").write_text(
            json.dumps(items, ensure_ascii=False, indent=1), encoding="utf-8"
        )
        manifest["categories"].append(
            {"id": cat, "title": CATEGORY_TITLES[cat], "count": len(items)}
        )
        for item in items:
            manifest["symbols"].append(
                {
                    "id": item["id"],
                    "name": item["name"],
                    "kind": item["kind"],
                    "category": cat,
                    "abstract": item["abstract"],
                    "conforms": item["conforms"],
                    "module": item["module"],
                }
            )

    (OUT_DIR / "index.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=1), encoding="utf-8"
    )

    documented = sum(1 for _s, d in records if d["abstract"])
    print(
        f"{len(records)} symbols across {len(manifest['categories'])} categories "
        f"({documented} with abstracts) -> {OUT_DIR}"
    )
    for cat in manifest["categories"]:
        print(f"  {cat['id']:12} {cat['count']:4}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
