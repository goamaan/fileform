#!/usr/bin/env python3
"""Validate the repository's simple ordered path filters and direct tool coverage.

This deliberately supports only the literal/*/** syntax used here, not arbitrary
GitHub glob/YAML syntax. Unsupported filter changes fail instead of being guessed.
"""
import ast
from pathlib import Path
import re

root = Path(__file__).resolve().parent.parent
workflows = {}
for name in ['ci', 'desktop']:
    text = (root / '.github/workflows' / (name + '.yml')).read_text()
    arrays = re.findall(r'^\s+paths:\s*(\[.*\])\s*$', text, re.MULTILINE)
    assert len(arrays) == 2, f'{name}: expected push and PR path lists'
    filters = [ast.literal_eval(value) for value in arrays]
    assert filters[0] == filters[1], f'{name}: push/PR coverage differs'
    assert 'workflow_dispatch:' in text, f'{name}: full manual verification must remain available'
    for pattern in filters[0]:
        assert isinstance(pattern, str) and not any(c in pattern for c in '?[]{}+\\'), f'Unsupported filter: {pattern}'
    workflows[name] = (filters[0], text)

def matches(name, path):
    included = False
    for pattern in workflows[name][0]:
        negative = pattern.startswith('!')
        if negative:
            pattern = pattern[1:]
        expression = '.*'.join(re.escape(part).replace(r'\*', '[^/]*') for part in pattern.split('**'))
        if re.fullmatch(expression, path):
            included = not negative
    return included

cases = {
    'Sources/FileformCore/ConversionEngine.swift': {'ci'},
    'Tests/FileformCoreTests/DocumentTableTests.swift': {'ci'},
    'Package.swift': {'ci'},
    'apps/macos/project.yml': {'ci'},
    'Tools/build-media-pack.sh': {'ci'},
    'Tools/smoke-editing.py': {'ci'},
    'crates/fileform-engine/src/lib.rs': {'desktop'},
    'Cargo.lock': {'desktop'},
    'rust-toolchain.toml': {'desktop'},
    'apps/desktop/src/image-workspace.tsx': {'desktop'},
    'Tools/smoke-portable.py': {'desktop'},
    'Tools/smoke-images.py': {'desktop'},
    'Tools/smoke-cancellation.py': {'desktop'},
    'Tools/check-ci-routing.py': {'ci', 'desktop'},
    'LICENSE': {'ci', 'desktop'},
    'NOTICE': {'ci', 'desktop'},
    'website/app/page.tsx': set(),
}
for path, expected in cases.items():
    assert (root / path).is_file(), f'Missing routing fixture: {path}'
    actual = {name for name in workflows if matches(name, path)}
    assert actual == expected, f'{path}: expected {expected}, got {actual}'

for name, (_, text) in workflows.items():
    for path in set(re.findall(r'\bTools/[A-Za-z0-9_./-]+\.(?:py|sh)\b', text.split('\njobs:', 1)[1])):
        assert (root / path).is_file(), f'{name}: referenced script is missing: {path}'
        assert matches(name, path), f'{name}: edits to invoked script do not trigger verification: {path}'
print(f'CI routing passed: {len(cases)} change classes and all directly invoked Tools scripts.')
