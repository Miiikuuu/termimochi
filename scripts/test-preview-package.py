#!/usr/bin/env python3
"""Install/uninstall the exact archive in private paths; never daily locations."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess as sp
import tarfile
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('archive', type=Path)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parent.parent
    base = repo / 'target/qa/preview-package'
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix='install-', dir=base))
    with tarfile.open(args.archive) as tar:
        tar.extractall(root / 'extracted', filter='data')
    package, = (root / 'extracted').iterdir()
    env = os.environ.copy()
    for key, folder in [('HOME', 'home'), ('XDG_DATA_HOME', 'data'), ('XDG_STATE_HOME', 'state'), ('XDG_CONFIG_HOME', 'config')]:
        env[key] = str(root / folder)
        (root / folder).mkdir()
    prefix = root / 'prefix with spaces'
    sentinel = root / 'config/sentinel'
    sentinel.write_text('unchanged')
    options = ['--prefix', str(prefix), '--data-home', env['XDG_DATA_HOME'], '--state-home', env['XDG_STATE_HOME']]
    with (root / 'install.log').open('w') as log:
        def run(*args):
            return sp.run(args, cwd=package, env=env, stdout=log, stderr=sp.STDOUT, check=True)
        run('bash', 'scripts/check-preview.sh')
        run('bash', 'scripts/install.sh', 'install', *options, '--dry-run')
        assert not prefix.exists()
        run('bash', 'scripts/install.sh', 'install', *options)
        binary = prefix / 'bin/termimochi'
        assert binary.read_bytes() == (package / 'target/release/termimochi').read_bytes()
        run(str(prefix / 'bin/termimochi-cli'), '--help')
        desktop = root / 'data/applications/io.github.miiikuuu.termimochi.desktop'
        run('desktop-file-validate', str(desktop))
        assert str(binary) in desktop.read_text()
        run('bash', 'scripts/install.sh', 'uninstall', *options)
        assert not binary.exists() and not desktop.exists()
        assert sentinel.read_text() == 'unchanged'
        # A damaged package must fail before any installation mutation.
        (package / 'target/release/termimochi-cli').write_bytes(b'broken')
        rejected = sp.run(['bash', 'scripts/install.sh', 'install', *options], cwd=package,
                          env=env, stdout=log, stderr=sp.STDOUT)
        assert rejected.returncode != 0 and not binary.exists()
    result = {'passed': True, 'archive': str(args.archive.resolve()),
              'sha256': hashlib.sha256(args.archive.read_bytes()).hexdigest(),
              'checks': ['integrity', 'dependencies', 'dry-run', 'isolated install', 'binary identity',
                         'CLI startup', 'desktop metadata', 'uninstall', 'config sentinel', 'tamper refusal'],
              'not_verified': ['real desktop menu', 'personal Conda/Bash', 'Wayland', 'multi-monitor']}
    (root / 'result.json').write_text(json.dumps(result, indent=2))
    print(root / 'result.json')


if __name__ == '__main__':
    main()
