#!/usr/bin/env python3
"""Regenerate notices with cargo-about 0.9.2 (install with --features cli)."""
import html
import json
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory() as directory:
    data_file = Path(directory) / 'licenses.json'
    subprocess.run(['cargo-about', 'generate', '--locked', '--all-features', '--format', 'json', '-o', str(data_file)], cwd=root, check=True)
    data = json.loads(data_file.read_text())
sections = []
for license in data['licenses']:
    packages = [entry.get('crate', entry.get('package', entry)) for entry in license['used_by']]
    names = ', '.join(f"{p['name']} {p['version']}" for p in packages)
    sections.append(f"<h2>{html.escape(license['name'])}</h2><p>{html.escape(names)}</p><pre>{html.escape(license['text'])}</pre>")
# Include upstream native source notices as well as their Rust wrappers.
native_roots = {'libgit2-sys': 'libgit2', 'libssh2-sys': 'libssh2', 'openssl-src': 'openssl', 'libz-sys': 'src/zlib'}
for crate in data['crates']:
    package = crate['package']
    if package['name'] not in native_roots:
        continue
    source = Path(package['manifest_path']).parent / native_roots[package['name']]
    for path in sorted(source.rglob('*')):
        if path.is_file() and (path.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE'))):
            text = path.read_text(errors='replace')
            title = f"{package['name']} {package['version']} / {path.relative_to(source)}"
            sections.append(f"<h2>{html.escape(title)}</h2><pre>{html.escape(text)}</pre>")
output = '''<!doctype html><html lang="en"><meta charset="utf-8"><title>SynchroGit third-party licenses</title>
<style>body{max-width:72rem;margin:2rem auto;padding:1rem;font:16px sans-serif}pre{white-space:pre-wrap}h2{margin-top:3rem}</style>
<h1>SynchroGit third-party licenses</h1>
<p>Generated from Cargo.lock for all supported platforms. Some components are platform-specific.
The Android interface also uses Kotlin and kotlinx.coroutines (JetBrains), and AndroidX/Jetpack Compose
(The Android Open Source Project), under the Apache License 2.0 reproduced below.</p>
''' + '\n'.join(sections) + '</html>\n'
(root / 'THIRD_PARTY_LICENSES.html').write_text(output)
print('Wrote THIRD_PARTY_LICENSES.html')
