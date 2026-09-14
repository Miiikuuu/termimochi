#!/usr/bin/env python3
"""Build a local, installable Linux preview. Never install, tag or publish it."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import shutil
import subprocess as sp
import tarfile
import tempfile
import tomllib


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--label', help='Archive label (defaults to the workspace version)')
    parser.add_argument('--no-build', action='store_true')
    args = parser.parse_args()
    repo = Path(__file__).resolve().parent.parent
    version = tomllib.loads((repo / 'Cargo.toml').read_text())['workspace']['package']['version']
    args.label = args.label or version
    if not args.label or any(c not in 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.-' for c in args.label):
        parser.error('label must contain only letters, digits, dots and hyphens')
    if not args.no_build:
        sp.run(['cargo', 'build', '--workspace', '--release', '--locked'], cwd=repo, check=True)
    output = repo / 'target/preview-packages'
    output.mkdir(parents=True, exist_ok=True)
    name = f'termimochi-{args.label}-linux-{platform.machine()}'
    archive = output / (name + '.tar.gz')
    if archive.exists():
        parser.error(f'refusing to overwrite {archive}; choose a new label')
    with tempfile.TemporaryDirectory(prefix='package-', dir=output) as temporary:
        root = Path(temporary) / name
        (root / 'scripts').mkdir(parents=True)
        (root / 'target/release').mkdir(parents=True)
        shutil.copytree(repo / 'data', root / 'data')
        shutil.copytree(repo / 'docs', root / 'docs')
        for script in ['install.sh', 'check-preview.sh']:
            shutil.copy2(repo / 'scripts' / script, root / 'scripts' / script)
        for doc in ['LICENSE', 'README.md']:
            shutil.copy2(repo / doc, root / doc)
        shutil.copy2(repo / 'docs/prerelease-install.md', root / 'INSTALL.md')
        dependencies = {}
        for binary in ['termimochi', 'termimochi-cli']:
            source = repo / 'target/release' / binary
            shutil.copy2(source, root / 'target/release' / binary)
            result = sp.run(['ldd', str(source)], text=True, capture_output=True, check=True)
            if 'not found' in result.stdout:
                raise RuntimeError(result.stdout)
            dependencies[binary] = result.stdout
        manifest = {
            'preview': args.label, 'workspace_version': version, 'architecture': platform.machine(),
            'build_os': Path('/etc/os-release').read_text(),
            'rustc': sp.check_output(['rustc', '--version'], text=True).strip(),
            'base_commit': sp.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip(),
            'dirty': bool(sp.check_output(['git', 'status', '--porcelain'], cwd=repo)),
            'feature': 'default (native-preview is not bundled)',
            'dynamic_dependencies': dependencies,
        }
        sources = sp.check_output(['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'], cwd=repo).split(b'\0')
        manifest['source_sha256'] = {os_path: hashlib.sha256((repo / os_path).read_bytes()).hexdigest()
                                    for os_path in sorted({p.decode() for p in sources if p})
                                    if (repo / os_path).is_file()}
        (root / 'PREVIEW.json').write_text(json.dumps(manifest, indent=2) + '\n')
        files = sorted(p for p in root.rglob('*') if p.is_file())
        (root / 'SHA256SUMS').write_text(''.join(
            f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.relative_to(root)}\n' for p in files))
        with tarfile.open(archive, 'w:gz') as tar:
            tar.add(root, arcname=name)
    checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_suffix(archive.suffix + '.sha256').write_text(f'{checksum}  {archive.name}\n')
    print(archive)
    print(checksum)


if __name__ == '__main__':
    main()
