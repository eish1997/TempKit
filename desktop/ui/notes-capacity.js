'use strict';
globalThis.quickgenNotesCapacity={async run(api){
 const invoke=window.__TAURI__.core.invoke,sleep=ms=>new Promise(r=>setTimeout(r,ms)),pending=new Map(),expected=new Map();
 const result={ok:false,kind:'notes-capacity',notes:100,residentWindows:10,durationTargetMs:1800000,edits:0,cycles:0,maxEditMs:0,maxLagMs:0,lagOver100ms:0,checks:[],physicalInputTest:false};
 let token=0,timer,unlisten,measurementStart;
 const require=(condition,message)=>{if(!condition)throw Error(message)};
 const checkVisible=async(expected,message)=>{const first=await invoke('notes_capacity_windows');if(first.visible===expected)return;result.windowTransitionSamples=[{afterMs:0,...first}];for(let i=1;i<=5;i++){await sleep(100);result.windowTransitionSamples.push({afterMs:i*100,...await invoke('notes_capacity_windows')})}throw Error(message)};
 try{
  require((await invoke('notes_list')).length===0,'Capacity requires a fresh empty isolated notes store');
  unlisten=await window.__TAURI__.event.listen('note-capacity-ack',e=>{const p=pending.get(e.payload.token);if(p&&p.id===e.payload.id){pending.delete(e.payload.token);clearTimeout(p.timeout);e.payload.error?p.reject(Error(e.payload.error)):p.resolve();}});
  const ids=[];
  for(let i=0;i<100;i++){let note=await invoke('note_create');const text=i===99?'':`容量便签 ${i+1}\n中文与多行正文\n`+'已确认保存内容。'.repeat(20);note=await invoke('note_patch',{id:note.id,patch:{field:'text',revision:note.text_revision,text}});expected.set(note.id,text);if(i>=10)await invoke('note_set_resident',{id:note.id,resident:false});else ids.push(note.id);}
  for(const id of ids)await invoke('note_open',{id});
  // Creation returns before page listeners are ready; require a full flush handshake.
  let ready=false;for(let attempt=0;attempt<3&&!ready;attempt++){try{await invoke('notes_flush');ready=true;}catch(error){if(attempt===2)throw error;}finally{await invoke('notes_release');}if(!ready)await sleep(500);}
  const windows=await invoke('notes_capacity_windows');require(windows.total===10&&windows.visible===10,'Expected exactly 10 visible note WebViews');
  const sceneBefore=await invoke('scene_snapshot');result.checks.push('100 persisted notes including one blank; exactly 10 resident WebViews');
  const start=measurementStart=performance.now();let last=start,nextEdit=0,nextCycle=0,nextReport=0;
  timer=setInterval(()=>{const now=performance.now(),lag=Math.max(0,now-last-250);last=now;result.maxLagMs=Math.max(result.maxLagMs,lag);if(lag>100)result.lagOver100ms++;},250);
  while(performance.now()-start<result.durationTargetMs){
   const elapsed=performance.now()-start;
   if(elapsed>=nextCycle){for(const note of await invoke('notes_list'))require(note.text===expected.get(note.id)&&!note.deleted,'Intermediate persisted text mismatch: '+note.id);await api.set(true);require(document.querySelectorAll('#notes-overview .note-tile').length===100,'Overview did not render 100 tiles');await checkVisible(0,'Editors remain visible in overview');await sleep(300);await api.set(false);await checkVisible(10,'Resident windows did not restore');result.cycles++;nextCycle=elapsed+30000;}
   if(elapsed>=nextEdit){const id=ids[result.edits%10],text=`持续编辑 ${result.edits}\n便签 ${id}\n`+'容量测试正文。'.repeat(20),n=++token,t=performance.now();
    const ack=new Promise((resolve,reject)=>{const timeout=setTimeout(()=>{pending.delete(n);reject(Error('Editor save acknowledgement timed out'));},10000);pending.set(n,{id,resolve,reject,timeout});});
    try{await invoke('notes_capacity_edit',{id,text,token:n});await ack;}catch(error){const p=pending.get(n);if(p){clearTimeout(p.timeout);pending.delete(n);}throw error;}
    result.maxEditMs=Math.max(result.maxEditMs,performance.now()-t);expected.set(id,text);result.edits++;nextEdit=elapsed+1000;
   }
   if(elapsed>=nextReport){result.elapsedMs=elapsed;await invoke('board_capacity_progress',{result});nextReport=elapsed+10000;}
   await sleep(100);
  }
  await invoke('notes_flush');const notes=await invoke('notes_list');require(notes.length===100,'Note count changed');for(const note of notes)require(note.text===expected.get(note.id)&&!note.deleted,'Persisted text mismatch');require(notes.filter(n=>n.resident).length===10,'Residency changed');
  require(JSON.stringify(await invoke('scene_snapshot'))===JSON.stringify(sceneBefore),'Image scene changed during notes cycles');
  const exported=await invoke('recovery_export');require(exported.manifest.noteCount===100,'Recovery export note count mismatch');result.exportDirectory=exported.directory;result.elapsedMs=performance.now()-start;result.checks.push('30 minutes, repeated overview transitions and per-editor save acknowledgements; exact final text and unchanged image scene; export includes 100 notes');result.ok=true;result.flushExit=true;
 }catch(error){result.error=String(error);}finally{if(measurementStart!==undefined)result.elapsedMs=performance.now()-measurementStart;if(timer)clearInterval(timer);unlisten?.();await invoke('board_smoke_finish',{result});}
}};
