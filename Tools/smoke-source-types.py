#!/usr/bin/env python3
"""Unsupported filesystem input types must fail promptly through the real CLI."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
root=Path(__file__).resolve().parent.parent
cli=root/'target/release'/('fileform-native'+('.exe' if sys.platform=='win32' else ''))
def inspect(path):
    return subprocess.run([str(cli),'inspect',str(path)],capture_output=True,timeout=3)
with tempfile.TemporaryDirectory(prefix='fileform-source-types-') as folder:
    base=Path(folder)
    source=base/'source.csv';source.write_bytes(b'name\nvalue\n')
    assert inspect(source).returncode==0
    directory=base/'directory.csv';directory.mkdir()
    assert inspect(directory).returncode!=0
    if os.name=='posix':
        pipe=base/'pipe.csv';os.mkfifo(pipe)
        failed=inspect(pipe)
        assert failed.returncode!=0 and json.loads(failed.stderr)['code']=='invalid_input'
        alias=base/'alias.csv';alias.symlink_to(source)
        assert inspect(alias).returncode==0
        pipe_alias=base/'pipe-alias.csv';pipe_alias.symlink_to(pipe)
        assert inspect(pipe_alias).returncode!=0
    else:
        failed=inspect(r'\\.\pipe\fileform-source.csv')
        assert failed.returncode!=0 and json.loads(failed.stderr)['code']=='invalid_input'
    assert source.read_bytes()==b'name\nvalue\n'
print('Source types: ordinary files accepted, directories/pipes rejected promptly, supported regular aliases preserved, source unchanged')
