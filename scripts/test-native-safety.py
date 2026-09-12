#!/usr/bin/python3
"""Private backend and immutable RC policy regression; no desktop needed."""
import importlib.util
import os
from pathlib import Path
import subprocess as sp
import tempfile
import unittest

repo=Path(__file__).resolve().parent.parent
prefix=repo/'target/native-preview/prefix'
spec=importlib.util.spec_from_file_location('policy',repo/'crates/termimochi/src/native_rc_auth.py')
policy=importlib.util.module_from_spec(spec); spec.loader.exec_module(policy)

class NativeSafety(unittest.TestCase):
    def allowed(self,cmd,payload,socket=True):
        return policy.is_cmd_allowed({'cmd':cmd,'payload':payload},None,socket,None)
    def test_owned_config_only(self):
        path=str(Path(policy.__file__).parent/'kitty.conf')
        self.assertTrue(self.allowed('load-config',{'paths':[path]}))
        self.assertFalse(self.allowed('load-config',{'paths':['/tmp/another-theme.conf']}))
    def test_tty_control_denied(self):
        self.assertFalse(self.allowed('load-config',{'paths':[str(Path(policy.__file__).parent/'kitty.conf')]},False))
    def test_arbitrary_commands_denied(self):
        for cmd in ['send-text','launch','run','set-colors','get-text','ls']:
            self.assertFalse(self.allowed(cmd,{}))
    def test_safety_overrides_cannot_be_removed(self):
        for extra in [{'ignore_overrides':True},{'override':['allow_remote_control=yes']}]:
            self.assertFalse(self.allowed('load-config',{'paths':[str(Path(policy.__file__).parent/'kitty.conf')],**extra}))
    def test_keyfile_isolation_and_batch_validation(self):
        with tempfile.TemporaryDirectory(prefix='tm-settings-test-') as folder:
            root=Path(folder); a=root/'a/keyfile'; b=root/'b/keyfile'
            env={'PATH':'/usr/bin:/bin','HOME':folder,'GSETTINGS_BACKEND':'memory','GSETTINGS_SCHEMA_DIR':str(prefix/'share/glib-2.0/schemas'),'LD_LIBRARY_PATH':f'{prefix}/lib:{prefix}/usr/lib/x86_64-linux-gnu'}
            def write(file,*values):
                return sp.run([str(prefix/'bin/termimochi-settings-helper'),str(file),'org.gnome.Ptyxis','-',*values],env=env,capture_output=True,timeout=5)
            self.assertEqual(write(a,'font-name',"'Monospace 13'").returncode,0)
            self.assertEqual(write(b,'font-name',"'Monospace 17'").returncode,0)
            before_a=a.read_bytes(); before_b=b.read_bytes()
            self.assertNotEqual(write(a,'font-name',"'Monospace 21'",'not-a-schema-key','true').returncode,0)
            self.assertEqual(a.read_bytes(),before_a)
            self.assertEqual(b.read_bytes(),before_b)
            self.assertEqual(write(a,'font-name',"'Noto Sans Mono CJK SC 14'").returncode,0)
            self.assertEqual(b.read_bytes(),before_b)
            self.assertEqual(write(a,'font-name','@reset').returncode,0)
            self.assertNotIn(b'font-name=',a.read_bytes() if a.exists() else b'')
            self.assertFalse((root/'config/dconf').exists())
    def test_relative_backend_rejected(self):
        run=sp.run([str(prefix/'bin/termimochi-settings-helper'),'relative-file','org.gnome.Ptyxis','-','font-name',"'Monospace 11'"],capture_output=True,timeout=5)
        self.assertEqual(run.returncode,2)

if __name__=='__main__': unittest.main(verbosity=2)
