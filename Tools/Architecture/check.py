#!/usr/bin/env python3
"""Check capability ownership and production import boundaries without building Swift."""
from collections import defaultdict
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[2]
# These are capability boundaries. Platform/native requirements do not define a layer.
PACKAGES = {
    'Core': (0, set()),
    'Text': (1, {'Core'}),
    'Imaging': (2, {'Core'}),
    'Graphics': (3, {'Core', 'Text', 'Imaging'}),
    'Shell': (4, set()),
    'Compose/Composition': (5, {'Core'}),
    'Compose': (5, {'Core', 'Text', 'Imaging', 'Compose/Composition'}),
    'Runtime': (6, {'Core', 'Text', 'Imaging', 'Graphics', 'Shell', 'Compose'}),
    'DevTools/Protocol': (7, set()),
    'DevTools/Inspection': (7, {'Core', 'DevTools/Protocol'}),
    'DevTools': (7, {'Core', 'Graphics', 'Runtime', 'DevTools/Protocol', 'DevTools/Inspection', 'Compose/Composition'}),
    '.': (8, {'Core', 'Text', 'Imaging', 'Graphics', 'Shell', 'Compose', 'Compose/Composition', 'Runtime', 'DevTools'}),
}
SUPPORT = {'System', 'Logging', 'PlatformCore'}
BRIDGES = {'CYoga', 'CFreeType', 'CHarfBuzz', 'CSDL3', 'CImageDecodeBridge', 'CDX12Bridge', 'CVulkanHeaders', 'GhosttyVt', 'SDL3', 'lunasvg', 'plutovg', 'webp', 'sharpyuv'}
# Cross-module names under the same capability remain implementation details.
DECLARATION = re.compile(r'\.(?:target|binaryTarget|macro|executableTarget|testTarget)\s*\(\s*name:\s*"([^"]+)"')
IMPORT = re.compile(r'^\s*(?:@_exported\s+|@_spi\([^)]*\)\s+|public\s+|internal\s+|private\s+|@preconcurrency\s+|@testable\s+|@_implementationOnly\s+)*import\s+(?:(?:typealias|struct|class|enum|protocol|let|var|func)\s+)?(\w+)', re.M)


def check():
    errors = []
    owners = defaultdict(list)
    modules = {}
    for package in PACKAGES:
        manifest = ROOT / package / 'Package.swift'
        if not manifest.is_file():
            errors.append(f'missing capability manifest: {manifest.relative_to(ROOT)}')
            continue
        source = manifest.read_text()
        for match in DECLARATION.finditer(source):
            if source[match.end():].lstrip().startswith(", condition:"):
                continue
            name = match.group(1)
            owners[name].append(package)
            modules[name] = package
        for product in re.findall(r'\.library\(\s*name:\s*"([^"]+)"', source):
            if product in BRIDGES or product in {'CoveUIScene', 'CoveUIComposeCore', 'NativeRHI', 'PlatformShell', 'CoveUIDevToolsScene', 'CoveUIDevToolsServer', 'CoveUIAgentCore', 'CoveUIAgentTransport'}:
                errors.append(f'{package}: implementation module exported as product {product}')
        for relative in re.findall(r'\.package\(\s*(?:name:\s*"[^"]+",\s*)?path:\s*"([^"]+)"', source):
            dependency = (manifest.parent / relative).resolve().relative_to(ROOT).as_posix()
            if dependency in PACKAGES and dependency not in PACKAGES[package][1]:
                errors.append(f'{package}: forbidden package dependency on {dependency}')
            elif dependency not in PACKAGES and dependency not in SUPPORT:
                errors.append(f'{package}: unclassified local dependency {dependency}')
    for name, packages in owners.items():
        if len(packages) != 1:
            errors.append(f'{name}: declared more than once: {", ".join(packages)}')
    for package in PACKAGES:
        sources = ROOT / package / 'Sources'
        if package == '.':
            folders = [ROOT / 'Sources', ROOT / 'Hosts/Desktop/Sources/CoveUI', ROOT / 'Hosts/Android/Integration', ROOT / 'Terminal/Sources/CoveUITerminal']
        else:
            folders = [sources]
        for folder in folders:
            for path in folder.rglob('*.swift'):
                for imported in IMPORT.findall(path.read_text(encoding='utf-8-sig')):
                    owner = modules.get(imported)
                    if owner is None or owner == package:
                        continue
                    if imported in BRIDGES:
                        errors.append(f'{path.relative_to(ROOT)}: imports another capability\'s bridge {imported}')
                    elif owner not in PACKAGES[package][1]:
                        errors.append(f'{path.relative_to(ROOT)}: forbidden import {imported} ({owner})')
    if (ROOT / 'Portable/Package.swift').exists():
        errors.append('deployment-based Portable package must not be restored')
    return errors, owners


if __name__ == '__main__':
    errors, owners = check()
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        sys.exit(1)
    print(f'Architecture OK: {len(PACKAGES)} capability/API packages; {len(owners)} targets have one owner; no upward imports or cross-capability bridge imports.')
