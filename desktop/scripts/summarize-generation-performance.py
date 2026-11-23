"""Summarize measured IPC/timer samples; never infer physical input latency."""
import json
import math
import sys
from collections import defaultdict
from pathlib import Path

evidence = Path(sys.argv[1]).resolve() if len(sys.argv)>1 else Path(__file__).resolve().parents[1] / 'evidence/generation/g2'
source = json.loads((evidence / 'generation-performance-latest.json').read_text(encoding='utf-8-sig'))
groups = defaultdict(list)
host_groups = defaultdict(list)
operation_groups = defaultdict(list)
memory = []
for run in source['runs']:
    assert run['exitCode'] == 0 and run['result']['ok'], 'Incomplete run'
    for sample in run['result']['samples']:
        groups[(run['mode'], sample['phase'], sample['name'])].append(sample['elapsed_ms'])
    for sample in run['result']['heartbeat']:
        groups[(run['mode'], sample['phase'], 'timer_interval')].append(sample['elapsed_ms'])
    host = run['result'].get('hostTimings', {})
    assert not host.get('truncated'), 'Incomplete host timing buffer'
    for sample in host.get('samples', []):
        host_groups[(run['mode'], sample['name'])].append(sample['ms'])
        if sample.get('operation'):
            operation_groups[(run['mode'], sample['operation'], sample['name'].rsplit('.',1)[-1])].append(sample['ms'])
    # A child can exit between enumeration and property reads. Preserve the
    # raw sample, but never silently interpret a missing counter as zero.
    valid_memory = [sample for sample in run['memory'] if sample['processes'] and
                    all(isinstance(p.get('privateBytes'), (int, float)) and p['privateBytes'] >= 0 for p in sample['processes'])]
    totals = [sum(p['privateBytes'] for p in sample['processes']) for sample in valid_memory]
    windows = {}
    for name, span in run['result'].get('memoryWindows', {}).items():
        selected = [sum(p['privateBytes'] for p in sample['processes']) / 1048576
                    for sample in valid_memory
                    if sample.get('startEpochMs', -1) >= span['start_ms']
                    and sample.get('endEpochMs', float('inf')) <= span['end_ms']]
        windows[name] = dict(duration_ms=span['end_ms']-span['start_ms'], samples=len(selected),
                             first_mib=selected[0] if selected else None,
                             last_mib=selected[-1] if selected else None,
                             min_mib=min(selected) if selected else None,
                             max_mib=max(selected) if selected else None,
                             delta_mib=selected[-1]-selected[0] if len(selected)>1 else None)
    memory.append(dict(mode=run['mode'], repeat=run['repeat'], windows=windows, sample_count=len(totals),
                       incomplete_samples=len(run['memory'])-len(valid_memory),
                       sampled_peak_private_mib=max(totals, default=0) / 1048576,
                       last_sample_private_mib=(totals[-1] if totals else 0) / 1048576))
rows = []
for (mode, phase, metric), values in sorted(groups.items()):
    values.sort()
    rows.append(dict(mode=mode, phase=phase, metric=metric, count=len(values),
                     p50_ms=values[math.ceil(len(values) * .5) - 1],
                     p95_ms=values[math.ceil(len(values) * .95) - 1], max_ms=values[-1]))
host_rows = []
for (mode, metric), values in sorted(host_groups.items()):
    values.sort()
    host_rows.append(dict(mode=mode, metric=metric, count=len(values),
                          p50_ms=values[math.ceil(len(values) * .5) - 1],
                          p95_ms=values[math.ceil(len(values) * .95) - 1], max_ms=values[-1]))
operation_rows = []
for (mode, operation, metric), values in sorted(operation_groups.items()):
    values.sort()
    operation_rows.append(dict(mode=mode, operation=operation, metric=metric, count=len(values),
                               p50_ms=values[math.ceil(len(values)*.5)-1], p95_ms=values[math.ceil(len(values)*.95)-1], max_ms=values[-1]))
result = dict(sha256=source['sha256'], runs=len(source['runs']), latency=rows, host=host_rows, operations=operation_rows, memory=memory,
              limitations=source['limitations'] + [
                  'Nearest-rank percentiles; sparse phases are not reliable tail estimates.',
                  'Timer intervals include intended 16 ms sleep, not just scheduling delay.',
                  'Last memory sample may coincide with shutdown and is not proof of reclamation.',
                  'Window samples must be fully inside WebView wall-clock intervals; short windows do not prove absence of leaks.',
                  'No historic baseline, pass/fail threshold, or physical input result inferred.'])
(evidence / 'generation-performance-summary.json').write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding='utf-8')
print(json.dumps(result, ensure_ascii=False, indent=2))
