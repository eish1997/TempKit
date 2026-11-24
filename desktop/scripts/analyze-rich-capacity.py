"""Analyze complete process-tree samples; private bytes and summed WS are distinct."""
import json,pathlib,statistics,sys
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
BASE=pathlib.Path(__file__).resolve().parents[1]/'evidence'/'notes-rich-migration'
RUNS=[('Event transport A','bff4ebee49ac4a34a78d1568c2435ab6'),('Event transport B','4d54445c3e0f4b50a9049b224f3379d9'),('IPC Channel','6b8972bde71644b583f0d64c46acec20')]
partial='--partial' in sys.argv
nodes='--nodes' in sys.argv
reuse='--reuse' in sys.argv or nodes
driver='--driver' in sys.argv or reuse
if driver:RUNS.append(('Channel + targeted test data','5c2dfd7e16274a788bd07a46e286c78f'))
if reuse:RUNS.append(('Channel + targeted data + reused text nodes','63689cafdda147ceb76c1d2603ae18a7'))
if nodes:RUNS.append(('Channel + all unchanged text nodes reused','e71e5bec1c79497e80ee0b224fda02ce'))
def read(p):return json.loads(p.read_text(encoding='utf-8-sig'))
def quantile(a,p):
    a=sorted(a);return a[min(len(a)-1,round((len(a)-1)*p))] if a else None
reports=[];fig,axes=plt.subplots(2,1,figsize=(10,7),sharex=True)
for label,suffix in RUNS:
    root=BASE/('notes-rich-capacity-'+suffix);identity=read(pathlib.Path(str(root)+'.identity.json'))
    samples=[]
    for line in pathlib.Path(str(root)+'.samples.jsonl').read_text(encoding='utf-8-sig').splitlines():
        try:r=json.loads(line)
        except json.JSONDecodeError:
            if partial:continue
            raise
        if not r['processes']:continue
        samples.append({'seconds':r['elapsedSeconds'],'private':sum(p['privateBytes'] for p in r['processes'])/1048576,'ws':sum(p['workingSetBytes'] for p in r['processes'])/1048576,'count':len(r['processes'])})
    resultPath=root/'board-smoke-result.json';result=read(resultPath) if resultPath.exists() else read(root/'capacity-progress.json')
    report={'label':label,'identity':str(root)+'.identity.json','sha256':identity['sha256'],'sampleCount':len(samples),'lastSampleSeconds':samples[-1]['seconds'],'privatePeakMiB':max(r['private'] for r in samples),'workingSetSumPeakMiB':max(r['ws'] for r in samples),'processesPeak':max(r['count'] for r in samples),'bins':[],'resultOk':result.get('ok',False),'error':result.get('error'),'edits':result.get('edits'),'elapsedMs':result.get('elapsedMs'),'editMedianMs':statistics.median(result['editSamples']) if result.get('editSamples') else None,'editP95Ms':quantile(result.get('editSamples',[]),.95),'editMaxMs':result.get('maxEditMs')}
    for low in range(0,int(samples[-1]['seconds'])+1,60):
        bucket=[r for r in samples if low<=r['seconds']<low+60]
        if bucket:report['bins'].append({'startSeconds':low,'privateMedianMiB':statistics.median(r['private'] for r in bucket),'workingSetSumMedianMiB':statistics.median(r['ws'] for r in bucket)})
    for key in ['private','ws']:
        tail=[r[key] for r in samples if samples[-1]['seconds']-300<=r['seconds']<=samples[-1]['seconds']-3]
        report[key+'Last5MinMedianMiB']=statistics.median(tail)
    finalFrames=result.get('finalInk',{}).get('frames',{})
    report['canvasCpuDrawSamples']={k:{'count':v['count'],'meanMs':v['total_ms']/v['count'] if v['count'] else None,'maxMs':v['max_ms'],'retainedP95Ms':quantile(v['samples'],.95)} for k,v in finalFrames.items()}
    if label==RUNS[-1][0] and not partial:
        assert identity['status']=='exited' and identity['exitCode']==0,'candidate did not exit successfully'
        assert result['ok'] and result['elapsedMs']>=1800000 and result['edits']>1000 and result['cycles']>=59
        assert result['finalInk']['objects']==550 and all(w['transparent'] for w in result['finalInk']['windows'])
        assert len(result['finalNotes'])==100
    reports.append(report)
    axes[0].plot([r['seconds']/60 for r in samples],[r['private']/1024 for r in samples],label=label,linewidth=1)
    axes[1].plot([r['seconds']/60 for r in samples],[r['ws']/1024 for r in samples],label=label,linewidth=1)
axes[0].set_ylabel('Private bytes sum (GiB)');axes[1].set_ylabel('Working set sum (GiB)');axes[1].set_xlabel('Minutes since test process start')
for ax in axes:ax.grid(alpha=.2);ax.legend();ax.set_ylim(bottom=0)
fig.suptitle('100 rich notes / 10 editors / 500 strokes + 50 text objects\nEvent runs failed and were stopped; latest run '+('in progress' if partial else 'completed'))
fig.tight_layout();name=('m7-node-final-capacity-' if nodes else 'm7-reuse-capacity-' if reuse else 'm7-driver-capacity-' if driver else 'm7-capacity-')+('partial' if partial else 'final');fig.savefig(BASE/(name+'.png'),dpi=150)
zoom,axis=plt.subplots(figsize=(10,4))
for report in reports[2:]:
    bins=[r for r in report['bins'] if r['startSeconds']>=60]
    axis.plot([r['startSeconds']/60 for r in bins],[r['privateMedianMiB'] for r in bins],label=report['label'],linewidth=1.3)
axis.set_xlabel('Minutes since test process start');axis.set_ylabel('Process-tree private bytes (MiB)')
axis.set_title('Channel variants: one-minute medians after initial startup\nOverlapping runs and other build/recovery work; no forced GC')
axis.grid(alpha=.2);axis.legend(fontsize=8);zoom.tight_layout();zoom.savefig(BASE/(name+'-steady.png'),dpi=150)
out={'complete':not partial,'scope':'Unselected Windows desktops on same machine; synthetic host mutations and real WebViews, no physical input. Event A/B overlapped; Channel run later. Other build/recovery work overlapped. WS sum counts shared pages repeatedly, not unique physical RAM. Canvas timing is CPU draw submission, not input latency or presentation frame rate.','runs':reports}
(BASE/(name+'.json')).write_text(json.dumps(out,ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps([{'label':r['label'],'minutes':round(r['lastSampleSeconds']/60,1),'privatePeakMiB':round(r['privatePeakMiB']),'privateLast5MinMedianMiB':round(r['privateLast5MinMedianMiB']),'resultOk':r['resultOk']} for r in reports]))
