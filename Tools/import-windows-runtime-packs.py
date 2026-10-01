#!/usr/bin/env python3
"""Import development tool packs from successful, recipe-matching main CI runs."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--repository',required=True)
parser.add_argument('--destination',type=Path,required=True)
args=parser.parse_args()
if sys.platform!='win32':raise RuntimeError('Run this import on the Windows packaging host')
root=Path(__file__).resolve().parents[1];destination=args.destination.resolve()
if destination.exists():raise FileExistsError('Use a new import directory')
destination.mkdir(parents=True)
def command(values,json_result=False):
 result=subprocess.run(values,cwd=root,capture_output=True,text=True,timeout=600)
 if result.returncode:raise RuntimeError('CI tool-pack import command failed: '+result.stderr[:1000])
 return json.loads(result.stdout) if json_result else result.stdout.strip()
def api(path):return command(['gh','api',path],True)
def recipe_matches(sha,files):
 for file in files:
  local=command(['git','rev-parse','HEAD:'+file])
  remote=api('repos/'+args.repository+'/contents/'+file+'?ref='+sha)
  if remote.get('sha')!=local:return False
 return True
entries=[
 ('media','media-windows.yml','fileform-media-windows-x64',['crates/fileform-engine/tools/build-media-windows.sh'],['MediaPack-Windows']),
 ('pdf','pdf-windows.yml','fileform-pdf-windows-x64',['crates/fileform-engine/tools/build-pdf-windows.sh','native/pdf-render/main.cpp','native/pdf-render/CMakeLists.txt'],['PDFPack-Windows','PDFRenderPack-Windows']),
 ('ocr','ocr-windows.yml','fileform-ocr-evaluation-windows-x64',['Tools/build-ocr-evaluation.py'],['OCRPack']),
 ('heic','heic-windows.yml','fileform-heic-dependency-evaluation-windows-x64',['Tools/build-heic-evaluation.py','native/heic-decode/main.cpp','native/heic-decode/CMakeLists.txt','native/heic-decode/stage-evaluation.py'],['HEICPack-Windows']),
]
records={};paths={}
for kind,workflow,artifact_name,recipes,packs in entries:
 runs=api('repos/'+args.repository+'/actions/workflows/'+workflow+'/runs?branch=main&event=push&status=success&per_page=30')['workflow_runs']
 selected=None
 for run in runs:
  if run.get('head_repository',{}).get('full_name')!=args.repository or run.get('head_branch')!='main' or run.get('status')!='completed' or run.get('conclusion')!='success':continue
  artifacts=api('repos/'+args.repository+'/actions/runs/'+str(run['id'])+'/artifacts')['artifacts']
  if not any(v['name']==artifact_name and not v.get('expired') for v in artifacts):continue
  if not recipe_matches(run['head_sha'],recipes):continue
  selected=run;break
 if selected is None:raise RuntimeError('No successful recipe-matching '+kind+' pack run; complete that native workflow first.')
 imported=destination/kind
 command(['gh','run','download',str(selected['id']),'--repo',args.repository,'--name',artifact_name,'--dir',str(imported)])
 for pack in packs:
  matches=[v for v in imported.rglob(pack) if v.is_dir() and (v/'manifest.json').is_file()]
  if len(matches)!=1:raise RuntimeError('Expected one complete '+pack+' in the imported artifact')
  paths[pack]=str(matches[0])
 records[kind]={'run':selected['id'],'commit':selected['head_sha'],'workflow':workflow,'recipes':recipes}
(destination/'imports.json').write_text(json.dumps({'repository':args.repository,'runs':records,'paths':paths},indent=2)+'\n',encoding='utf-8')
print(destination/'imports.json')
