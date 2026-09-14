#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Real lossy PDF CLI workflows and independent expected-object graph proof."""
import argparse
import base64
import hashlib
from importlib.util import spec_from_file_location, module_from_spec
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
_spec = spec_from_file_location('optimization_fixtures', Path(__file__).with_name('generate-pdf-optimization-fixtures.py'))
_fixture = module_from_spec(_spec); _spec.loader.exec_module(_fixture)


def canonical(document):
    objects = document['qpdf'][1]; seen = {}
    def visit(value, stream=False, trailer=False):
        if isinstance(value, str) and 'obj:' + value in objects:
            if value in seen: return ['ref', seen[value]]
            index = len(seen); seen[value] = index
            return ['object', index, visit(objects['obj:' + value])]
        if isinstance(value, list): return [visit(item) for item in value]
        if isinstance(value, dict):
            ignored = {'/Length'} if stream else set()
            if trailer:
                ignored |= {'/ID', '/Size', '/Prev', '/XRefStm'}
                if value.get('/Type') == '/XRef': ignored |= {'/Type', '/W', '/Index', '/Length', '/Filter', '/DecodeParms'}
            return {key: visit(item, stream=key == 'dict' and 'data' in value) for key, item in sorted(value.items()) if key not in ignored}
        return value
    return visit(objects['trailer']['value'], trailer=True)


def exercise(work):
    _fixture.generate(work)
    source = work / 'landscape.pdf'; original_hash = hashlib.sha256(source.read_bytes()).hexdigest()
    pack = ROOT / 'Artifacts/PDFPack'; qpdf = pack / 'bin/qpdf'
    env = dict(os.environ, FILEFORM_PDF_PACK=str(pack))
    history = []
    def run(*args, error=None, record=None):
        completed = subprocess.run([str(ROOT/'.build/debug/fileform'), *map(str, args)], env=env, text=True, capture_output=True, timeout=240)
        try: value = json.loads(completed.stdout)
        except Exception: raise AssertionError((completed.returncode, completed.stdout, completed.stderr))
        if error: assert completed.returncode != 0 and value['code'] == error, value
        else: assert completed.returncode == 0, (value, completed.stderr)
        if record: (work/record).write_text(json.dumps(value, indent=2)+'\n')
        history.append({'command': list(map(str, args)), 'exitCode': completed.returncode, 'status': value.get('status', value.get('code'))})
        return value
    plan = run('pdf', 'optimize', source, '--output', work/'CLI optimized.pdf', '--maximum-image-dimension', 480, '--dry-run', '--json', record='plan.json')
    assert not (work/'CLI optimized.pdf').exists()
    details = plan['pdfOptimization']
    assert details['optimizedImageCount'] == 2 and details['resampledImageCount'] == 1
    assert len([c for c in details['candidates'] if c.get('skipReason')]) == 1
    compressed = run('pdf', 'optimize', source, '--output', work/'CLI optimized.pdf', '--maximum-image-dimension', 480, '--json', record='compression.json')
    assert compressed['status'] == 'succeeded'
    output = work/'CLI optimized.pdf'
    assert output.stat().st_size < source.stat().st_size
    inspected = run('inspect', output, '--worker', ROOT/'.build/debug/fileform-worker', '--json')
    assert inspected['pageCount'] == 3

    def inventory(path):
        return json.loads(subprocess.check_output([qpdf, path, '--json', '--json-key=qpdf', '--json-stream-data=inline', '--decode-level=generalized']))
    before, after = inventory(source), inventory(output)
    def resource_image(document, name):
        objects = document['qpdf'][1]
        root = objects['trailer']['value']['/Root']
        pages = objects['obj:'+objects['obj:'+root]['value']['/Pages']]['value']
        resources = pages.get('/Resources')
        if resources is None:
            page = objects['obj:'+pages['/Kids'][0]]['value']; resources = page['/Resources']
        if isinstance(resources, str): resources = objects['obj:'+resources]['value']
        return resources['/XObject'][name]
    for name in ['/Photo', '/Gray']:
        ref_before, ref_after = resource_image(before, name), resource_image(after, name)
        replacement = after['qpdf'][1]['obj:'+ref_after]['stream']
        expected = before['qpdf'][1]['obj:'+ref_before]['stream']
        assert replacement['dict']['/Filter'] == '/DCTDecode'
        expected['dict'].update({key: replacement['dict'][key] for key in ['/Filter', '/Width', '/Height']})
        expected['dict'].pop('/DecodeParms', None)
        expected['data'] = replacement['data']
    a, b = canonical(before), canonical(after)
    assert a == b, 'Non-image object, content, metadata, resource or geometry changed'
    graph_hash = hashlib.sha256(json.dumps(a, sort_keys=True).encode()).hexdigest()

    floor = run('pdf', 'optimize', source, '--output', work/'CLI floor.pdf', '--quality', .35, '--minimum-quality', .35,
                '--maximum-image-dimension', 480, '--json', record='floor.json')
    floor_bytes = floor['artifacts'][0]['bytes']
    fitted = run('pdf', 'optimize', source, '--output', work/'CLI exact fit.pdf', '--quality', .95, '--minimum-quality', .35,
                 '--maximum-image-dimension', 480, '--max-bytes', floor_bytes, '--json', record='fit.json')
    assert fitted['artifacts'][0]['bytes'] == floor_bytes and fitted['attempts'] == 6
    assert fitted['pdfOptimization']['usedQuality'] == .35
    miss = run('pdf', 'optimize', source, '--output', work/'CLI missed.pdf', '--quality', .95, '--minimum-quality', .35,
               '--maximum-image-dimension', 480, '--max-bytes', 1, '--json', error='target_unmet', record='target-miss.json')
    attempted = miss['pdfOptimization']['attemptedQualities']
    assert len(attempted) == 6 and attempted[0] == .95 and attempted[-1] == .35 and min(attempted) >= .35
    assert 'usedQuality' not in miss['pdfOptimization'] and not (work/'CLI missed.pdf').exists()

    # Typed transform and portable setup preserve all optimization options.
    request = plan['request']; request['output']['destination'] = (work/'CLI transformed.pdf').as_uri()
    (work/'request.json').write_text(json.dumps(request, indent=2)+'\n')
    transformed = run('transform', work/'request.json', '--json', record='transform.json')
    assert transformed['pdfOptimization']['resampledImageCount'] == 1
    recipe = run('setup', 'create', work/'request.json', '--name', 'PDF image email', '--json', record='setup.json')
    assert recipe['operation'] == request['operation'] and str(work) not in json.dumps(recipe)
    applied = run('setup', 'apply', work/'setup.json', '--asset', 'source='+str(source), '--output', work/'CLI setup.pdf', '--json', record='setup-result.json')
    assert applied['status'] == 'succeeded' and applied['pdfOptimization']['usedQuality'] == .8
    run('pdf', 'optimize', source, '--output', output, '--json', error='destination_exists')
    renamed = run('pdf', 'optimize', source, '--output', output, '--maximum-image-dimension', 480, '--collision', 'rename', '--json')
    assert renamed['artifacts'][0]['url'].endswith('CLI%20optimized-1.pdf')
    alias = work/'alias.pdf'; os.link(source, alias)
    run('pdf', 'optimize', source, '--output', alias, '--json', error='invalid_request')

    # A compact plain PDF with no images proves the no-publication result.
    plain = work/'plain.pdf'
    _fixture._module.pdf(plain, [b'<< /Type /Catalog /Pages 2 0 R >>', b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>', b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Resources << >> >>'])
    compact = work/'compact.pdf'
    subprocess.run([qpdf, '--object-streams=generate', '--compress-streams=y', '--recompress-flate', '--compression-level=9', plain, compact], check=True)
    no_smaller = run('pdf', 'optimize', compact, '--output', work/'CLI no-smaller.pdf', '--json', record='not-smaller.json')
    assert no_smaller['status'] == 'not_smaller' and not no_smaller['artifacts'] and not (work/'CLI no-smaller.pdf').exists()
    assert hashlib.sha256(source.read_bytes()).hexdigest() == original_hash
    assert not list(work.glob('.fileform-*'))
    evidence = {'sourceSHA256': original_hash, 'sourceBytes': source.stat().st_size, 'outputBytes': output.stat().st_size,
        'pages': 3, 'optimizedImages': 2, 'resampledImages': 1, 'retainedUnsupportedImages': 1,
        'expectedReachableGraphSHA256': graph_hash, 'independentGraphProof': True,
        'floorBytes': floor_bytes, 'fitAttempts': fitted['attempts'], 'fitQualities': fitted['pdfOptimization']['attemptedQualities'],
        'targetMiss': True, 'notSmaller': True, 'portableSetup': True, 'collisionAndAliasSafety': True, 'sourceRetained': True, 'scratchClean': True}
    (work/'verification.json').write_text(json.dumps(evidence, indent=2)+'\n')
    (work/'commands.json').write_text(json.dumps(history, indent=2)+'\n')
    print('PDF image CLI passed: complete graph, all pages, quality-floor retries/exact fit/miss, resampling, skips, setup/transform, not-smaller and safe publication.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('--work', type=Path); args = parser.parse_args()
    if args.work:
        args.work.mkdir(parents=True, exist_ok=False); exercise(args.work.resolve())
    else:
        with tempfile.TemporaryDirectory(prefix='fileform-pdf-optimize-cli-') as directory: exercise(Path(directory))
