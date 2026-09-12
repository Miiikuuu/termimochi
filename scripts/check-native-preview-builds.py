#!/usr/bin/python3
"""Check both build boundaries, retaining exact logs and separate artifacts."""
import hashlib
import json
import os
from pathlib import Path
import subprocess as sp
import tempfile

repo=Path(__file__).resolve().parent.parent
out=Path(tempfile.mkdtemp(prefix='build-checks-',dir=repo/'target/qa/native-preview'))
env=os.environ.copy()
prefix=repo/'target/native-preview/prefix'
env.update(RUSTUP_HOME=str(repo/'target/qa/typed-toolchain/rustup'),RUSTUP_TOOLCHAIN='1.92.0',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',PKG_CONFIG_PATH=f'{prefix}/lib/pkgconfig:{prefix}/usr/lib/x86_64-linux-gnu/pkgconfig',LD_LIBRARY_PATH=f'{prefix}/lib:{prefix}/usr/lib/x86_64-linux-gnu')
results=[]
def check(name,argv):
    with (out/(name+'.log')).open('w') as log:
        result=sp.run(argv,cwd=repo,env=env,stdout=log,stderr=sp.STDOUT,check=False)
    results.append({'name':name,'command':argv,'exit':result.returncode})
    (out/'results.json').write_text(json.dumps(results,indent=2))
    print(name,result.returncode,flush=True)
    return result.returncode==0
print('Evidence:',out,flush=True)
check('fmt',['cargo','fmt','--all','--','--check'])
for mode,feature in [('default',[]),('enabled',['--features','native-preview'])]:
    check(mode+'-clippy',['cargo','clippy','--workspace','--all-targets','--locked',*feature,'--','-D','warnings'])
    check(mode+'-tests',['cargo','test','--workspace','--locked',*feature])
    if check(mode+'-release',['cargo','build','--workspace','--release','--locked',*feature]):
        binary=out/('termimochi-'+mode)
        binary.write_bytes((repo/'target/release/termimochi').read_bytes()); binary.chmod(0o700)
        (out/(mode+'-sha256.txt')).write_text(hashlib.sha256(binary.read_bytes()).hexdigest()+'\n')
        if mode=='default':
            check('installer',['bash','scripts/test-install.sh'])
            check('metadata-tests',['python3','-m','unittest','discover','-s','scripts','-p','test_desktop_resources.py','-v'])
            check('metadata',['python3','scripts/validate_desktop_resources.py'])
raise SystemExit(0 if all(r['exit']==0 for r in results) else 1)
