#!/usr/bin/python3
"""Enabled-feature native tests on an owned Xvfb/private Fcitx session."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess as sp
import tempfile
import time

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--scale',type=int,action='append',choices=[1,2])
parser.add_argument('--filter',action='append',default=[])
parser.add_argument('--display',default=':98')
args=parser.parse_args()
repo=Path(__file__).resolve().parent.parent
prefix=repo/'target/native-preview/prefix'
out=Path(tempfile.mkdtemp(prefix='termimochi-regression-native-',dir=repo/'target/qa/native-preview'))
env=os.environ.copy()
env.update(TERMIMOCHI_NATIVE_PREFIX=str(prefix),PKG_CONFIG_PATH=f'{prefix}/lib/pkgconfig:{prefix}/usr/lib/x86_64-linux-gnu/pkgconfig',LD_LIBRARY_PATH=f'{prefix}/lib:{prefix}/usr/lib/x86_64-linux-gnu',RUSTUP_HOME=str(repo/'target/qa/typed-toolchain/rustup'),RUSTUP_TOOLCHAIN='1.92.0',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0')
print('Evidence:',out,flush=True)
build=sp.run(['cargo','test','-p','termimochi','--features','native-preview','--locked','--no-run','--message-format=json'],cwd=repo,env=env,stdout=sp.PIPE,text=True,check=False)
(out/'build.jsonl').write_text(build.stdout)
if build.returncode:
    for item in map(json.loads,build.stdout.splitlines()):
        if item.get('reason')=='compiler-message': print(item['message'].get('rendered',''))
    raise SystemExit(build.returncode)
binary=next(Path(item['executable']) for item in map(json.loads,build.stdout.splitlines()) if item.get('reason')=='compiler-artifact' and item.get('executable') and item['profile']['test'] and item['target']['name']=='termimochi')
pinned=out/'termimochi-tests'; pinned.write_bytes(binary.read_bytes()); pinned.chmod(0o700)
worker_build=sp.run(['cargo','build','-p','termimochi','--features','native-preview','--locked','--message-format=json'],cwd=repo,env=env,stdout=sp.PIPE,text=True,check=True)
worker_binary=next(Path(item['executable']) for item in map(json.loads,worker_build.stdout.splitlines()) if item.get('reason')=='compiler-artifact' and item.get('executable') and item['target']['name']=='termimochi')
worker=out/'termimochi-worker'; worker.write_bytes(worker_binary.read_bytes()); worker.chmod(0o700)
env['TERMIMOCHI_SVG_WORKER_BIN']=str(worker)
input_driver=out/'native-input.py'; input_driver.write_bytes((repo/'scripts/native-preview-input.py').read_bytes())
env['TERMIMOCHI_NATIVE_INPUT_DRIVER']=str(input_driver)
(out/'build.json').write_text(json.dumps({'features':['native-preview'],'sha256':hashlib.sha256(pinned.read_bytes()).hexdigest(),'worker_sha256':hashlib.sha256(worker.read_bytes()).hexdigest(),'casilda_sha256':hashlib.sha256((prefix/'lib/libcasilda-1.0.so.1.0').read_bytes()).hexdigest(),'ptyxis_sha256':hashlib.sha256((prefix/'bin/ptyxis').read_bytes()).hexdigest(),'input_driver_sha256':hashlib.sha256((repo/'scripts/native-preview-input.py').read_bytes()).hexdigest()},indent=2))
listing=sp.run([str(pinned),'--list','--ignored'],env=env,capture_output=True,text=True,check=True).stdout
tests=[line.removesuffix(': test') for line in listing.splitlines() if line.endswith(': test') and any(f in line for f in args.filter or ['native_interactive_'])]
assert tests,'No tests selected'
assert args.display.startswith(':') and args.display[1:].isdecimal() and int(args.display[1:])>1,'Use an isolated display'
display_socket=Path('/tmp/.X11-unix/X'+args.display[1:])
assert not display_socket.exists(),'Do not reuse another display'
xvfb_binary=next((repo/'target/qa/native-tools').glob('*/runtime/usr/bin/Xvfb'))
xvfb=sp.Popen([str(xvfb_binary),args.display,'-screen','0','3200x2200x24','-nolisten','tcp','-noreset'],stdout=sp.DEVNULL,stderr=sp.DEVNULL)
results=[]
try:
    for _ in range(100):
        if display_socket.exists(): break
        time.sleep(.05)
    for scale in args.scale or [1,2]:
        for test in tests:
            case=out/f'{scale}x-{test.split("::")[-1]}'; case.mkdir()
            case_env=env.copy()
            for k in ['WAYLAND_DISPLAY','WAYLAND_SOCKET','DBUS_SESSION_BUS_ADDRESS','BASH_ENV','ENV','STARSHIP_CONFIG','PTYXIS_PROFILE','SSH_AUTH_SOCK']:
                case_env.pop(k,None)
            for key,folder in [('HOME','home'),('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_CACHE_HOME','cache'),('XDG_STATE_HOME','state')]:
                (case/folder).mkdir(); case_env[key]=str(case/folder)
            runtime=Path(tempfile.mkdtemp(prefix='tm-native-qa-'))
            case_env.update(XDG_RUNTIME_DIR=str(runtime),DISPLAY=args.display,GDK_BACKEND='x11',GSK_RENDERER='cairo',GSETTINGS_BACKEND='memory',GTK_A11Y='none',GTK_IM_MODULE='fcitx',GIO_USE_VFS='local',GDK_SCALE=str(scale),LIBGL_ALWAYS_SOFTWARE='1')
            (case/'config/fcitx5').mkdir()
            (case/'config/fcitx5/profile').write_text('[Groups/0]\nName=Default\nDefault Layout=us\nDefaultIM=pinyin\n[Groups/0/Items/0]\nName=keyboard-us\nLayout=\n[Groups/0/Items/1]\nName=pinyin\nLayout=\n[GroupOrder]\n0=Default\n')
            started=time.monotonic()
            with (case/'output.log').open('w') as log:
                child=sp.Popen(['dbus-run-session','--','bash','--noprofile','--norc','-c','fcitx5 -D --disable=wayland,waylandim,notificationitem,notifications,cloudpinyin & exec "$@"','native-qa',str(pinned),test,'--exact','--ignored','--test-threads=1','--nocapture'],cwd=repo,env=case_env,stdout=log,stderr=sp.STDOUT,start_new_session=True)
                try: code=child.wait(timeout=180)
                except sp.TimeoutExpired:
                    os.killpg(child.pid,signal.SIGTERM)
                    try: child.wait(timeout=5)
                    except sp.TimeoutExpired: os.killpg(child.pid,signal.SIGKILL); child.wait()
                    code=124
                finally:
                    try: os.killpg(child.pid,signal.SIGTERM)
                    except ProcessLookupError: pass
            passed=code==0 and '1 passed; 0 failed' in (case/'output.log').read_text()
            result={'test':test,'scale':scale,'passed':passed,'exit':code,'seconds':round(time.monotonic()-started,2),'log':str(case/'output.log')}
            results.append(result); (out/'results.json').write_text(json.dumps(results,indent=2))
            print(json.dumps(result),flush=True)
finally:
    xvfb.terminate(); xvfb.wait(timeout=5)
raise SystemExit(0 if all(r['passed'] for r in results) else 1)
