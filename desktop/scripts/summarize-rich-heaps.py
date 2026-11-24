"""Read-only heap/DOM samples, separate from process-tree private/WS accounting."""
from pathlib import Path
import json,statistics
base=Path(__file__).resolve().parents[1]/'evidence'/'notes-rich-migration'
reports=[]
for label,suffix in [('targeted-data','5c2dfd7e16274a788bd07a46e286c78f'),('reused-text-nodes','63689cafdda147ceb76c1d2603ae18a7'),('all-unchanged-text-reused','e71e5bec1c79497e80ee0b224fda02ce')]:
    path=base/('notes-rich-capacity-'+suffix+'.heaps.jsonl')
    raw=[json.loads(line) for line in path.read_text(encoding='utf-8-sig').splitlines()]
    rows=[]
    for sample in raw:
        if 'pages' not in sample:continue
        notes=[p for p in sample['pages'] if '/note.html' in p['url']]
        rows.append({'seconds':sample['elapsedSeconds'],'notes':len(notes),'noteJsUsedMiB':sum(p['heap']['usedSize'] for p in notes)/1048576,'noteJsReservedMiB':sum(p['heap']['totalSize'] for p in notes)/1048576,'noteEmbedderUsedMiB':sum(p['heap'].get('embedderHeapUsedSize',0) for p in notes)/1048576,'noteNodes':sum(p['dom']['nodes'] for p in notes),'noteListeners':sum(p['dom']['jsEventListeners'] for p in notes)})
    report={'label':label,'samplePath':str(path),'samples':rows,'connectionErrors':[r for r in raw if 'error' in r]}
    if rows:
        report['first']=rows[0];report['last']=rows[-1]
        report['fiveMinuteBins']=[]
        for start in range(0,int(rows[-1]['seconds'])+1,300):
            bucket=[r for r in rows if start<=r['seconds']<start+300]
            if bucket:report['fiveMinuteBins'].append({'startSeconds':start,**{k:{'min':min(r[k] for r in bucket),'median':statistics.median(r[k] for r in bucket),'max':max(r[k] for r in bucket)} for k in ['noteJsUsedMiB','noteJsReservedMiB','noteEmbedderUsedMiB','noteNodes','noteListeners']}})
    reports.append(report)
out={'scope':'Three actual isolated native WebView process groups. Runtime.getHeapUsage and Memory.getDOMCounters only, no forced GC or evaluation. Some CDP connection failures are retained; sampler restart did not restart either test app. JS heap is not process private bytes; DOM counters can include unreachable nodes awaiting automatic collection. Explicit-GC probe exists only in a separate mocked-host headless fixture. Runs partly overlap.','runs':reports}
name='m7-native-heap-trends.json'
(base/name).write_text(json.dumps(out,ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps([{k:r.get(k) for k in ['label','first','last']} for r in reports]))
