"""Read-only bounded idle comparison; never infer latency or whole-stage acceptance."""
import hashlib
import json
import pathlib
import statistics
import sys

root = pathlib.Path(sys.argv[1]).resolve()
protocol_path = root / 'protocol.json'
protocol = json.loads(protocol_path.read_text(encoding='utf-8'))
digest = hashlib.sha256(protocol_path.read_bytes()).hexdigest()
metrics = {}
for variant in ['old', 'current']:
    identity = json.loads((root / (variant + '.identity.json')).read_text(encoding='utf-8-sig'))
    assert identity['protocolSHA256'].lower() == digest, 'Protocol changed after sampling began'
    assert identity['status'] in ['sampled-awaiting-normal-exit', 'exited'], 'Incomplete sample'
    assert identity['sha256'] == protocol['variants'][variant]['sha256']
    samples = [json.loads(line) for line in (root / (variant + '.samples.jsonl')).read_text(encoding='utf-8-sig').splitlines() if line.strip()]
    samples = [s for s in samples if s['elapsed'] >= protocol['warmupSeconds']]
    assert len(samples) >= 60 and samples[-1]['elapsed'] - samples[0]['elapsed'] >= 170, 'Insufficient sampling coverage'
    memory = [(s['elapsed'], sum(p['private'] for p in s['processes']) / 1048576) for s in samples]
    cpu, elapsed = 0.0, 0.0
    for previous, current in zip(samples, samples[1:]):
        before = {(p['pid'], p['start']): p['cpu'] for p in previous['processes']}
        cpu += sum(max(0, p['cpu'] - before[(p['pid'], p['start'])]) for p in current['processes'] if (p['pid'], p['start']) in before)
        elapsed += current['elapsed'] - previous['elapsed']
    first = statistics.mean(m for t, m in memory if t <= memory[0][0] + 30)
    last = statistics.mean(m for t, m in memory if t >= memory[-1][0] - 30)
    metrics[variant] = dict(samples=len(samples), seconds=elapsed, meanSingleCoreCpuPercent=100 * cpu / elapsed,
                            peakPrivateMiB=max(m for _, m in memory), first30MeanMiB=first, last30MeanMiB=last,
                            tailGrowthMiB=last-first)
old, current = metrics['old'], metrics['current']
limits = dict(meanSingleCoreCpuPercent=max(old['meanSingleCoreCpuPercent']*1.25, old['meanSingleCoreCpuPercent']+2),
              peakPrivateMiB=max(old['peakPrivateMiB']*1.25, old['peakPrivateMiB']+100), tailGrowthMiB=32)
checks = {name: dict(actual=current[name], maximum=limit, passed=current[name] <= limit) for name, limit in limits.items()}
result = dict(protocolSHA256=digest, metrics=metrics, checks=checks, boundedIdleMetricsPassed=all(c['passed'] for c in checks.values()),
              wholePerformanceGatePassed=False, limitations=protocol['limitations'] + ['Process lifetime matching excludes short-lived workers between samples.'])
target = root / 'summary.json'
assert not target.exists(), 'Preserve prior report'
target.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding='utf-8')
print(json.dumps(result, ensure_ascii=False, indent=2))
