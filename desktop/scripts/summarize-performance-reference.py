"""Summarize completed reference runs without treating CPU as input latency."""
import json
import statistics
import sys
from pathlib import Path

root = Path(sys.argv[1]).resolve()
manifest = json.loads((root/'manifest.json').read_text(encoding='utf-8-sig'))
runs = []
failures = []
for identity_file in sorted(root.glob('*.identity.json')):
    identity = json.loads(identity_file.read_text(encoding='utf-8-sig'))
    if identity['status'] != 'exited' or identity.get('exitCode') != 0 or not identity.get('result',{}).get('ok'):
        failures.append({'identity':identity_file.name,'status':identity['status'],'exitCode':identity.get('exitCode'),'result':identity.get('result')})
        continue
    rows = [json.loads(line) for line in Path(identity['samples']).read_text(encoding='utf-8-sig').splitlines() if line.strip()]
    metrics = []
    for previous, current in zip(rows, rows[1:]):
        if previous['phase'] != current['phase']:
            continue
        dt = current['elapsedSeconds'] - previous['elapsedSeconds']
        before = {(p['pid'],p['startTime']): p['cpuSeconds'] for p in previous['processes']}
        cpu = sum(max(0,p['cpuSeconds']-before[(p['pid'],p['startTime'])]) for p in current['processes'] if (p['pid'],p['startTime']) in before)
        metrics.append({'phase':current['phase'],'dt':dt,'cpu':cpu,'privateMiB':sum(p['privateBytes'] for p in current['processes'])/1048576})
    phases = {}
    for name in sorted(set(m['phase'] for m in metrics)):
        group = [m for m in metrics if m['phase']==name]
        phases[name] = {'intervals':len(group),'seconds':sum(m['dt'] for m in group),
                        'singleCoreCPUPercent':100*sum(m['cpu'] for m in group)/sum(m['dt'] for m in group),
                        'privateMiBMean':statistics.mean(m['privateMiB'] for m in group),
                        'privateMiBPeak':max(m['privateMiB'] for m in group)}
    runs.append({'identity':identity_file.name,'variant':identity['variant'],'sha256':identity['sha256'],'result':identity['result'],'processTree':phases})
assert {r['variant'] for r in runs} == {'legacy','durable'}, 'Both completed variants required'
assert all(r['result']['sample'] == runs[0]['result']['sample'] for r in runs), 'Sample differs'
out = {'runs':runs,'excludedIncompleteOrFailedRuns':failures,'limitation':manifest['limitation']+' CPU counts only matched live process lifetimes; short decoder processes between samples are excluded. Order and other desktop workloads are not controlled. Generated outputs differ by implementation; completion time includes the old intentional timer. This is not the 30-minute final-candidate gate.'}
(root/'summary.json').write_text(json.dumps(out,indent=2),encoding='utf-8')
print(json.dumps(out,indent=2))
