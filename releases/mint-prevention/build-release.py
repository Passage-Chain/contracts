#!/usr/bin/env python3
"""Build historical release workspaces; no chain/network/signing operations.

Prerequisite: task-local Rust 1.69.0 and wasm target in ROOT/rust/{cargo,rustup}.
Pinned crate downloads can occur through Cargo; use --offline for cached dependencies.
"""
import argparse,hashlib,json,os,shutil,subprocess
from pathlib import Path

parser=argparse.ArgumentParser()
parser.add_argument('--task-root',required=True,type=Path)
parser.add_argument('--offline',action='store_true')
args=parser.parse_args()
root=args.task_root.resolve();source=Path(__file__).resolve().parent
if root == Path('/') or source == root or source.is_relative_to(root/'contracts'):
    parser.error('Use a dedicated build directory outside repository source.')
env=os.environ.copy()
env.update(CARGO_HOME=str(root/'rust/cargo'),RUSTUP_HOME=str(root/'rust/rustup'),CARGO_REGISTRIES_CRATES_IO_PROTOCOL='sparse')
env['PATH']=str(root/'rust/cargo/bin')+':'+env['PATH']
env['RUSTFLAGS']=f'-C target-cpu=mvp --remap-path-prefix={root}=/build'
expected=dict(line.split()[::-1] for line in (source/'checksums.sha256').read_text().splitlines())
out=root/'artifacts';out.mkdir(parents=True,exist_ok=True)
for family in ('v020','v030'):
    env['CARGO_TARGET_DIR']=str(root/('target-'+family))
    workspace=root/'contracts'/family
    shutil.copytree(source/family,workspace,dirs_exist_ok=True)
    command=['cargo','+1.69.0','build','--locked','--release','--target','wasm32-unknown-unknown','-p','pg721-metadata-onchain','--lib']
    if args.offline:command.append('--offline')
    subprocess.run(command,cwd=workspace,env=env,check=True)
    binary=Path(env['CARGO_TARGET_DIR'])/'wasm32-unknown-unknown/release/pg721_metadata_onchain.wasm'
    name='retired-'+family+'.wasm';sha=hashlib.sha256(binary.read_bytes()).hexdigest()
    if sha != expected[name]:raise SystemExit(f'{name}: checksum mismatch {sha}; do not use earlier runtime validation for changed bytes')
    shutil.copyfile(binary,out/name)
    testenv=env.copy();testenv.pop('RUSTFLAGS',None)
    command=['cargo','+1.69.0','test','--locked','-p','pg721-metadata-onchain','--test','retirement']
    if args.offline:command.append('--offline')
    subprocess.run(command,cwd=workspace,env=testenv,check=True)
    print(name,sha,flush=True)
