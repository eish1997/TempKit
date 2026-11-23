"""Apply a frozen prospective protocol; keep failures and sparse evidence visible."""
import json
import pathlib
import sys

root = pathlib.Path(sys.argv[1]).resolve()
protocol = json.loads((root/'protocol.json').read_text(encoding='utf-8'))
summary = json.loads((root/'generation-performance-summary.json').read_text(encoding='utf-8'))
raw = json.loads((root/'generation-performance-latest.json').read_text(encoding='utf-8-sig'))
limits = protocol['thresholds']
assert summary['runs'] == protocol['repeats'] * len(protocol['modes'])
assert {(r['mode'], r['repeat']) for r in raw['runs']} == {(m, i) for m in protocol['modes'] for i in range(1, protocol['repeats']+1)}
checks = []
def check(row, metric, maximum, minimum=1):
    sufficient = row['count'] >= minimum
    checks.append(dict(mode=row['mode'], phase=row.get('phase'), operation=row['metric'], statistic=metric,
                       samples=row['count'], value=row[metric], maximum=maximum,
                       status='insufficient' if not sufficient else ('passed' if row[metric] <= maximum else 'failed')))

for row in summary['latency']:
    if row['metric'] in ['feed', 'query', 'note']:
        check(row, 'p95_ms', limits['ipc_feed_query_note_p95_ms'], limits['minimum_samples_for_percentile'])
        check(row, 'max_ms', limits['ipc_feed_query_note_max_ms'])
    elif row['metric'] == 'result_decode':
        check(row, 'max_ms', limits['result_decode_'+row['mode']+'_max_ms'])
for row in summary['host']:
    if row['metric'].endswith('.queue'):
        check(row, 'p95_ms', limits['host_queue_p95_ms'], limits['minimum_samples_for_percentile'])
        check(row, 'max_ms', limits['host_queue_max_ms'])
for mode in protocol['modes']:
    assert any(r['mode']==mode and r['metric']=='result_decode' for r in summary['latency']), 'Missing result decode'
    for metric in ['feed','query','note']:
        assert any(r['mode']==mode and r['metric']==metric and r['phase']=='running' for r in summary['latency']), 'Missing running-phase IPC'
        assert any(r['mode']==mode and r['metric']==metric+'.queue' for r in summary['host']), 'Missing host queue timing'
report = dict(candidate=summary['sha256'], checks=checks,
              failures=[c for c in checks if c['status']=='failed'], insufficient=[c for c in checks if c['status']=='insufficient'],
              latencyProtocolPassed=all(c['status']=='passed' for c in checks), wholeStagePassed=False,
              limitations=['Diagnostic companion and local fake service; no old-release latency comparison or physical IME result inferred.',
                           'Task queries are measured; this does not claim resource_read latency coverage.'])
path=root/'latency-verdict.json'
assert not path.exists(), 'Preserve existing verdict'
path.write_text(json.dumps(report, indent=2), encoding='utf-8')
print(json.dumps({k:report[k] for k in ['candidate','failures','insufficient','latencyProtocolPassed']}, indent=2))
