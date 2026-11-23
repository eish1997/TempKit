"""Generate isolated UI build inputs; never replace product UI or production data."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import uuid

desktop = Path(__file__).resolve().parents[1]
root = desktop / 'evidence/reliable-base/r4' / ('comparison-' + uuid.uuid4().hex)
root.mkdir()
legacy = desktop / 'evidence/reliable-base/r1/board-before-snapshot.js'
assert hashlib.sha256(legacy.read_bytes()).hexdigest() == '52eaafd0edad9cf808019c753407f3a02f5991d1a41ee93ff067455ee0f9ec4e'
harness = (desktop / 'ui/board-capacity.js').read_text(encoding='utf-8')
seed = harness[:harness.index(' async run(api){')]
run = r''' async run(api){
  const invoke=window.__TAURI__.core.invoke,start=performance.now();
  const result={ok:false,sample:JSON.parse(localStorage.getItem('capacity-sample')),phases:[]};
  let phase={name:'processing',startMs:0,lags:[]},last=performance.now();result.phases.push(phase);
  const timer=setInterval(()=>{const now=performance.now();phase.lags.push(Math.max(0,now-last-250));last=now;},250);
  try{
   if(api.items.length!==100)throw Error('Expected identical 100 image seed');
   api.show(true);api.pass(true,true);
   api.simulate(api.items.slice(0,50),api.cards.get('clay'));
   const deadline=Date.now()+180000;
   while(api.jobs.filter(j=>j.state==='done').length!==50||api.items.length!==150){
    if(Date.now()>deadline)throw Error('50 task timeout');await new Promise(r=>setTimeout(r,250));await api.sync();
   }
   await api.save();phase.endMs=performance.now()-start;result.processingMs=phase.endMs;
   phase={name:'idle-passthrough',startMs:phase.endMs,lags:[]};result.phases.push(phase);
   await invoke('board_capacity_progress',{result:{elapsedMs:performance.now()-start,phase:phase.name,items:api.items.length,jobs:api.jobs.length}});
   const idleStart=performance.now();
   while(performance.now()-idleStart<180000){
    await new Promise(r=>setTimeout(r,10000));
    await invoke('board_capacity_progress',{result:{elapsedMs:performance.now()-start,phase:phase.name,items:api.items.length,jobs:api.jobs.length}});
   }
   await api.sync();await api.save();phase.endMs=performance.now()-start;
   if(api.items.length!==150||api.jobs.length!==50||api.jobs.some(j=>j.state!=='done'))throw Error('Final scene mismatch');
   result.items=150;result.jobs=50;result.ok=true;
  }catch(error){result.error=String(error)}finally{
   clearInterval(timer);result.elapsedMs=performance.now()-start;
   for(const p of result.phases){p.samples=p.lags.length;p.maxLagMs=Math.max(0,...p.lags);p.over100ms=p.lags.filter(n=>n>100).length;delete p.lags;}
   await invoke('board_smoke_finish',{result});
  }
 }
};
'''
for variant in ['legacy', 'durable']:
    ui = root / variant / 'ui'
    shutil.copytree(desktop / 'ui', ui)
    (ui / 'board-capacity.js').write_text(seed + run, encoding='utf-8')
    if variant == 'legacy':
        source = legacy.read_text(encoding='utf-8')
        # Restore normal legacy persistence in diagnostic mode. Shared host is
        # retained only for isolation, native window handling and measurement.
        source = source.replace('if(diagnostic||!db||!storageReady)return;', 'if(!db||!storageReady)return;')
        anchor = 'await initStorage();applyTheme(initialConfig.theme);'
        assert source.count(anchor) == 1
        before = "await new Promise((resolve,reject)=>{const s=document.createElement('script');s.src='board-capacity.js';s.onload=resolve;s.onerror=reject;document.head.append(s)});await quickgenBoardCapacity.seed();\n"
        source = source.replace(anchor, before + anchor)
        end = '\n})().catch(error=>'
        assert source.count(end) == 1
        hook = "\nquickgenBoardCapacity.run({items,jobs,cards,simulate,sync:async()=>{},save:async()=>saveScene(),hide:hideCanvas,show:showCanvas,pass:setPassThrough});\n"
        source = source.replace(end, hook + end)
        (ui / 'board.js').write_text(source, encoding='utf-8')
    (root / variant / 'tauri-config.json').write_text(json.dumps({'build': {'frontendDist': os.path.relpath(ui, desktop/'src-tauri').replace('\\','/')}}), encoding='utf-8')
manifest = {'format': 'quickgen-performance-reference-v1', 'root': str(root),
            'legacyBoardSHA256': hashlib.sha256(legacy.read_bytes()).hexdigest(),
            'limitation': 'Old board source with normal IndexedDB saves on current isolated native host; not the old published binary. Both variants use the same diagnostic workload. No native input latency claim.',
            'productSources': {str(p.relative_to(desktop)): hashlib.sha256(p.read_bytes()).hexdigest() for p in [*sorted((desktop/'src-tauri/src').rglob('*.rs')), desktop/'src-tauri/Cargo.toml', desktop/'src-tauri/Cargo.lock', *sorted((desktop/'ui').glob('*'))] if p.is_file()}}
(root / 'manifest.json').write_text(json.dumps(manifest,indent=2),encoding='utf-8')
print(root)
