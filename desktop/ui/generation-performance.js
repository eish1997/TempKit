'use strict';
globalThis.quickgenGenerationPerformance={async run(api,mode){
 const invoke=window.__TAURI__.core.invoke,sleep=ms=>new Promise(r=>setTimeout(r,ms));
 const samples=[],heartbeat=[],memoryWindows={},start=performance.now();let phase='setup',stop=false,probe,clock,note,probeError;
 const timed=async(name,fn)=>{const begin=performance.now(),atPhase=phase;const value=await fn();samples.push({name,phase:atPhase,start_ms:begin-start,elapsed_ms:performance.now()-begin});return value;};
 try{
  await invoke('generation_test',{action:'setup'});await invoke('generation_test',{action:'clear'});await invoke('generation_test',{action:'download-clear'});
  note=await invoke('note_create');
  // Retain startup/fixture timing separately from the request workload. The
  // full-span trace remains available so prior failures are never hidden.
  await api.save();
  const setupHostTimings=await invoke('generation_test',{action:'performance-timings'});
  // No overlapping probe requests: the observer must not flood the bounded host queue.
  probe=(async()=>{let serial=0;while(!stop){await timed('feed',()=>invoke('task_feed',{}));await timed('query',()=>invoke('task_snapshot'));note=await timed('note',()=>invoke('note_patch',{id:note.id,patch:{field:'text',revision:note.text_revision,text:'isolated sample '+serial++}}));await sleep(25);}})().catch(error=>{probeError=error;stop=true;});
  clock=(async()=>{let previous=performance.now();while(!stop){await sleep(16);const now=performance.now();heartbeat.push({phase,at_ms:now-start,elapsed_ms:now-previous});previous=now;}})();
  phase='idle';memoryWindows.idle={start_ms:Date.now()};await sleep(3000);memoryWindows.idle.end_ms=Date.now();
  phase='running';api.simulate([api.items[0]],api.cards.get('clay'));
  const deadline=performance.now()+45000;let terminal=false;
  while(performance.now()<deadline){await api.sync();const tasks=await invoke('task_snapshot');if(tasks[0])phase=tasks[0].state;if(tasks[0]?.failure)throw Error('Unexpected fixture task failure');if(tasks[0]?.state==='succeeded'&&api.items.length===2&&!api.busy()){terminal=true;break;}await sleep(25);}
  if(!terminal)throw Error('Performance fixture timeout');
  const output=api.items.find(i=>i.taskId);const element=document.querySelector('[data-image="'+output.id+'"] img');if(!element)throw Error('Result image missing');await timed('result_decode',()=>element.decode());
  phase='settled';memoryWindows.settled={start_ms:Date.now()};await sleep(10000);memoryWindows.settled.end_ms=Date.now();stop=true;await Promise.all([probe,clock]);if(probeError)throw probeError;
  const stats=await invoke('generation_test',{action:'clear'});if(stats.requests!==1||stats.downloads!==1||stats.executions!==1)throw Error('Unexpected fixture execution count');
  const measurementHostTimings=await invoke('generation_test',{action:'performance-timings'});
  const hostTimings={samples:[...setupHostTimings.samples,...measurementHostTimings.samples],truncated:setupHostTimings.truncated||measurementHostTimings.truncated};
  if(hostTimings.truncated)throw Error('Host timing buffer filled; incomplete measurement');
  await api.save();await invoke('board_smoke_finish',{result:{ok:true,flushExit:true,mode,stats,samples,heartbeat,hostTimings,setupHostTimings,measurementHostTimings,memoryWindows,limitations:['Timer heartbeat is not physical input latency.','Phase labels are observed task states, not precise decoder/file-write boundaries.','This is a current-source reference, not the missing pre-G0 historical baseline.']}});
 }catch(error){stop=true;await Promise.allSettled([probe,clock]);await invoke('board_smoke_finish',{result:{ok:false,error:String(error),mode,samples,heartbeat}});}
}};
