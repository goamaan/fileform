#!/usr/bin/env python3
"""Targeted image recompression proves the complete expected object graph."""
import base64
import copy
import hashlib
from importlib.util import spec_from_file_location,module_from_spec
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time
from pdf_fixtures import fixture, write_pdf
root=Path(__file__).resolve().parents[3]
pdf,renderer=[Path(v).resolve() for v in sys.argv[1:]]
suffix='.exe' if sys.platform=='win32' else ''
cli=root/'target/release'/('fileform-native'+suffix);worker=root/'target/release'/('fileform-worker'+suffix)
qpdf=pdf/'bin'/('qpdf'+suffix)
spec=spec_from_file_location('reference_lossy_smoke',root/'Tools/smoke-pdf-image-optimization.py')
reference=module_from_spec(spec);spec.loader.exec_module(reference)
def run(*args):return subprocess.run([str(v) for v in args],capture_output=True,timeout=240)
def optimize(source,output,*args):return run(cli,'optimize-pdf-images',source,output,pdf,renderer,*args)
def inventory(path):
    result=run(qpdf,path,'--json','--json-key=qpdf','--json-stream-data=inline','--decode-level=generalized')
    assert result.returncode==0,result.stderr
    return json.loads(result.stdout)
def resource_image(document,name):
    objects=document['qpdf'][1];catalog=objects['obj:'+objects['trailer']['value']['/Root']]['value']
    pages=objects['obj:'+catalog['/Pages']]['value'];first=objects['obj:'+pages['/Kids'][0]]['value']
    resource=first.get('/Resources',pages.get('/Resources'))
    if isinstance(resource,str):resource=objects['obj:'+resource]['value']
    return resource['/XObject'][name]
def verify_expected_graph(source,output,candidates,names=None):
    before,after=inventory(source),inventory(output)
    expected=copy.deepcopy(before)
    for name in names or ['/Photo','/Gray']:
        old,new=resource_image(before,name),resource_image(after,name)
        replacement=after['qpdf'][1]['obj:'+new]['stream']
        entry=expected['qpdf'][1]['obj:'+old]['stream']
        number=int(old.split()[0]);candidate=next(c for c in candidates if c['object_number']==number)
        assert replacement['dict']['/Filter']=='/DCTDecode'
        assert (replacement['dict']['/Width'],replacement['dict']['/Height'])==(candidate['output_width'],candidate['output_height'])
        entry['dict'].update({key:replacement['dict'][key] for key in ['/Filter','/Width','/Height']})
        entry['dict'].pop('/DecodeParms',None);entry['data']=replacement['data']
        assert base64.b64decode(replacement['data']).startswith(b'\xff\xd8')
    assert reference.canonical(expected)==reference.canonical(after),'A non-image object, vector/content stream, metadata, unsupported image or page geometry changed'
with tempfile.TemporaryDirectory(prefix='fileform-lossy-pdf-') as folder:
    base=Path(folder);reference._fixture.generate(base,width=320)
    source=base/'landscape.pdf';original=source.read_bytes();planned=base/'planned.pdf'
    result=optimize(source,planned,'--max-dimension',160,'--dry-run')
    assert result.returncode==0,result.stderr
    plan=json.loads(result.stdout)
    assert plan['status']=='planned' and not planned.exists() and plan['attempted_quality']==[]
    assert len([c for c in plan['candidates'] if c['skip_reason'] is None])==2
    photo=next(c for c in plan['candidates'] if c['original_width']==320)
    assert (photo['output_width'],photo['output_height'])==(160,106) # Floor, not nearest rounding.
    output=base/'optimized.pdf'
    result=optimize(source,output,'--max-dimension',160)
    assert result.returncode==0,result.stderr
    receipt=json.loads(result.stdout)
    assert receipt['status']=='saved' and receipt['pages']==3 and receipt['output_bytes']<len(original)
    assert receipt['output_sha256']==hashlib.sha256(output.read_bytes()).hexdigest()
    assert receipt['source_sha256']==hashlib.sha256(original).hexdigest()
    verify_expected_graph(source,output,receipt['candidates'])
    low=base/'floor.pdf'
    result=optimize(source,low,'--quality',.35,'--minimum-quality',.35,'--max-dimension',160)
    assert result.returncode==0,result.stderr
    floor_bytes=low.stat().st_size
    fitted=base/'fit.pdf'
    result=optimize(source,fitted,'--quality',.95,'--minimum-quality',.35,'--max-dimension',160,'--max-bytes',floor_bytes)
    assert result.returncode==0,result.stderr
    fit=json.loads(result.stdout);assert fitted.stat().st_size<=floor_bytes
    assert fit['attempted_quality'][0]==.95 and fit['selected_quality']>=.35 and len(fit['attempted_quality'])<=6
    verify_expected_graph(source,fitted,fit['candidates'])
    failed=base/'target-miss.pdf'
    result=optimize(source,failed,'--quality',.83,'--minimum-quality',.505,'--max-bytes',10)
    assert result.returncode!=0 and not failed.exists()
    error=json.loads(result.stderr);assert error['code']=='target_unmet'
    assert len(error['pdf_optimization']['attempted_quality'])==6
    assert error['pdf_optimization']['attempted_quality'][-1]==.505 and error['pdf_optimization']['selected_quality'] is None
    assert source.read_bytes()==original
    preserved=output.read_bytes();assert optimize(source,output).returncode!=0 and output.read_bytes()==preserved
    request={'operation':'optimize_pdf_images','input':str(source),'output':str(base/'worker.pdf'),
             'directory':str(pdf),'renderer_directory':str(renderer),'max_dimension':160,'allow_lossy':True}
    response=subprocess.run([str(worker)],input=json.dumps(request)+'\n',text=True,capture_output=True,check=True,timeout=240)
    worker_receipt=json.loads(response.stdout)['result']
    verify_expected_graph(source,base/'worker.pdf',worker_receipt['candidates'])
    def object_stream(dictionary,data):
        return f'<< {dictionary} /Length {len(data)} >>\nstream\n'.encode()+data+b'\nendstream'
    masked=base/'masked.pdf'
    objects=[b'<< /Type /Catalog /Pages 2 0 R >>',b'<< /Type /Pages /Count 1 /Kids [3 0 R] >>',
             b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /XObject << /Masked 5 0 R /MaskAsPrimary 6 0 R /Plain 7 0 R >> >> /Contents 4 0 R >>',
             object_stream('',b'q 100 0 0 100 0 0 cm /Masked Do Q\n'),
             object_stream('/Type /XObject /Subtype /Image /Width 2 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB /SMask 6 0 R',bytes([255,0,0,0,0,255])),
             object_stream('/Type /XObject /Subtype /Image /Width 2 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceGray',bytes([0,127])),
             object_stream('/Type /XObject /Subtype /Image /Width 2 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB',bytes([30,60,90,90,60,30]))]
    write_pdf(masked,objects)
    result=optimize(masked,base/'masked-plan.pdf','--dry-run');assert result.returncode==0,result.stderr
    mask_plan=json.loads(result.stdout)
    assert all(c['skip_reason'] for c in mask_plan['candidates'] if c['object_number'] in (5,6))
    assert next(c for c in mask_plan['candidates'] if c['object_number']==7)['skip_reason'] is None
    # Exercise JPEG eligibility, retaining ICC-tagged bytes and rewriting only
    # a plain JPEG interpreted by its DeviceRGB PDF dictionary.
    import struct,zlib
    def chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
    image=base/'tiny.png'
    image.write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',2,1,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(b'\0'+bytes([255,0,0,0,0,255])))+chunk(b'IEND',b''))
    encoded=base/'tiny.jpg';assert run(cli,'convert-image',image,encoded).returncode==0
    tagged=encoded.read_bytes();plain=bytearray(tagged[:2]);offset=2
    while offset<len(tagged):
        marker=tagged[offset+1]
        if marker==0xda:plain.extend(tagged[offset:]);break
        length=struct.unpack('>H',tagged[offset+2:offset+4])[0];segment=tagged[offset:offset+length+2]
        if not (marker==0xe2 and segment[4:].startswith(b'ICC_PROFILE\0')):plain.extend(segment)
        offset+=length+2
    jpeg_pdf=base/'jpeg.pdf'
    jpeg_objects=objects[:5]
    jpeg_objects[2]=b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /XObject << /Tagged 5 0 R /Plain 6 0 R >> >> /Contents 4 0 R >>'
    jpeg_objects[3]=object_stream('',b'')
    jpeg_objects[4]=object_stream('/Type /XObject /Subtype /Image /Width 2 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB /Filter /DCTDecode',tagged)
    jpeg_objects.append(object_stream('/Type /XObject /Subtype /Image /Width 2 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB /Filter /DCTDecode',bytes(plain)))
    write_pdf(jpeg_pdf,jpeg_objects)
    result=optimize(jpeg_pdf,base/'jpeg-plan.pdf','--dry-run');assert result.returncode==0,result.stderr
    jpeg_plan=json.loads(result.stdout)
    assert next(c for c in jpeg_plan['candidates'] if c['object_number']==5)['skip_reason']
    assert next(c for c in jpeg_plan['candidates'] if c['object_number']==6)['skip_reason'] is None
    # Explicit fit targets publish even if a tiny fixture gets larger. Verify
    # retained masks and profiled JPEG bytes in the actual rewritten graph.
    for case in [masked,jpeg_pdf]:
        candidate=base/(case.stem+'-saved.pdf')
        result=optimize(case,candidate,'--max-bytes',100000)
        assert result.returncode==0,result.stderr
        verify_expected_graph(case,candidate,json.loads(result.stdout)['candidates'],['/Plain'])
    # A fully rewritten document with no images is retained when there is no
    # smaller result. No empty or redundant output should be published.
    clean=base/'clean.pdf';fixture(clean,text=True)
    compact=base/'compact.pdf'
    result=optimize(clean,compact,'--max-bytes',100000)
    assert result.returncode==0,result.stderr
    redundant=base/'redundant.pdf'
    result=optimize(compact,redundant)
    assert result.returncode==0 and json.loads(result.stdout)['status']=='not_smaller' and not redundant.exists(),result.stderr
    # Repeated pages make verification observable without sleeps as a trigger.
    # Cancel/change the source after the candidate PDF has actually been made.
    repeated=base/'repeated.pdf'
    assert run(qpdf,source,'--pages','.',','.join(['1']*16),'--',repeated).returncode==0
    bound=repeated.read_bytes()
    for action in ['cancel','source-change']:
        destination=base/(action+'.pdf');baseline=set(base.iterdir())
        process=subprocess.Popen([str(worker)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        try:
            active={**request,'input':str(repeated),'output':str(destination)}
            process.stdin.write(json.dumps({'request':active,'cancel_on_disconnect':True})+'\n');process.stdin.flush()
            deadline=time.monotonic()+90
            while not any(p.is_file() and p.stat().st_size>0 for p in set(base.iterdir())-baseline):
                assert process.poll() is None and time.monotonic()<deadline
                time.sleep(.01)
            assert not destination.exists()
            if action=='cancel':process.stdin.write('cancel\n');process.stdin.flush()
            else:repeated.write_bytes(bound+b'\n%source changed\n')
            process.wait(timeout=180)
            reply=json.loads(process.stdout.read());stderr=process.stderr.read()
            assert not reply['ok'] and reply['error']['code']==('cancelled' if action=='cancel' else 'source_changed'),(reply,stderr)
        finally:
            if process.poll() is None:process.kill();process.wait()
            process.stdin.close();process.stdout.close();process.stderr.close()
            repeated.write_bytes(bound)
        assert not destination.exists() and set(base.iterdir())==baseline
    annotated=base/'annotated.pdf';fixture(annotated,annotated=True)
    rejected=base/'rejected.pdf'
    assert optimize(annotated,rejected).returncode!=0 and not rejected.exists()
    no_images=base/'plain.pdf';fixture(no_images,text=True)
    target=base/'plain-target.pdf'
    result=optimize(no_images,target,'--max-bytes',10)
    assert result.returncode!=0 and len(json.loads(result.stderr)['pdf_optimization']['attempted_quality'])==1
print('Lossy PDF: real photographic/gray resampling, exact independent expected graph, all pages/metadata/vectors retained, floor rounding, original-based fit attempts, detailed target misses, CLI/worker, collisions and unsupported rejection passed')
