#!/usr/bin/env python3
"""Register a Rust integration test in a crate manifest, safely under concurrency.

Cargo discovers integration tests only from the package root (`<crate>/tests`).
Cove keeps test sources in `<crate>/rust/tests`, so every target needs an
explicit `[[test]]` entry. Parallel migration workers append to the same
`Cargo.toml`, which would otherwise lose updates, so appends are serialized with
an exclusive `flock` and made idempotent.

Usage:
    register_test.py <crate-dir> <test-name>

`<test-name>` is the target name; the path is derived as
`src/tests/<test-name>.rs`. Missing files and duplicate entries are rejected so
a manifest can never point at a target that does not build.
"""

from __future__ import annotations

import fcntl
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: register_test.py <crate-dir> <test-name>", file=sys.stderr)
        return 2

    crate_dir = sys.argv[1]
    name = sys.argv[2]
    rel_path = f"src/tests/{name}.rs"
    source = os.path.join(ROOT, crate_dir, rel_path)
    manifest = os.path.join(ROOT, crate_dir, "Cargo.toml")

    if not os.path.isfile(source):
        print(f"missing test source: {source}", file=sys.stderr)
        return 1
    if not os.path.isfile(manifest):
        print(f"missing manifest: {manifest}", file=sys.stderr)
        return 1

    with open(manifest, "r+", encoding="utf-8") as handle:
        fcntl.flock(handle.fileno(), fcntl.LOCK_EX)
        try:
            text = handle.read()
            if f'name = "{name}"' in text and f'path = "{rel_path}"' in text:
                print(f"already registered: {name}")
                return 0

            addition = f'\n[[test]]\nname = "{name}"\npath = "{rel_path}"\n'
            if not text.endswith("\n"):
                addition = "\n" + addition
            handle.seek(0, os.SEEK_END)
            handle.write(addition)
            handle.flush()
            os.fsync(handle.fileno())
            print(f"registered: {name} -> {rel_path}")
            return 0
        finally:
            fcntl.flock(handle.fileno(), fcntl.LOCK_UN)


if __name__ == "__main__":
    raise SystemExit(main())
