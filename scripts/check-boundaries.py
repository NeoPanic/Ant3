#!/usr/bin/env python3
"""Check workspace dependency boundaries, including transitive normal dependencies."""
import json
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[1]
metadata = json.loads(subprocess.check_output(
    ['cargo', 'metadata', '--locked', '--no-deps', '--format-version=1'], cwd=root,
))
packages = {p['name']: p for p in metadata['packages']}
graph = {name: {d['name'] for d in p['dependencies'] if d['kind'] != 'dev'}
         for name, p in packages.items()}

def closure(name, trail=()):
    assert name not in trail, f'normal dependency cycle: {trail + (name,)}'
    result = set(graph.get(name, ()))
    for child in graph.get(name, ()):
        if child in graph:
            result.update(closure(child, trail + (name,)))
    return result

for name, package in packages.items():
    assert 'test-support' not in package['features'].get('default', []), name
    for dependency in package['dependencies']:
        if dependency['kind'] != 'dev':
            assert 'test-support' not in dependency['features'], (name, dependency['name'])

for name in graph:
    closure(name)
for name in ['ant3-domain', 'ant3-client', 'ant3-models', 'ant3-plugins']:
    dependencies = closure(name)
    assert not dependencies & {'rusqlite', 'axum', 'ant3-platform', 'ant3-worker', 'ant3-server'}, (name, dependencies)
assert not closure('ant3-client') & {'ant3', 'ant3-plugins', 'ant3-models'}
assert not closure('ant3-domain') & {'ant3', 'reqwest', 'tokio'}
assert not closure('ant3-platform') & {'ant3-client', 'ant3-worker', 'ant3-server', 'ant3-cli'}
assert not closure('ant3-server') & {'ant3-cli'}
assert not closure('ant3-storage') & {'ant3-platform', 'ant3-server', 'ant3-client', 'ant3-worker', 'ant3-plugins'}
assert not closure('ant3-builtin-plugins') & {'ant3-platform', 'ant3-server', 'ant3-storage', 'rusqlite'}
assert any(t['name'] == 'ant3' and 'bin' in t['kind']
           for t in packages['ant3-cli']['targets'])
print('Workspace dependency boundaries PASS')
