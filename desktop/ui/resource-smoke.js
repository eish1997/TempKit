'use strict';
(async()=>{
 const invoke=window.__TAURI__.core.invoke;
 const result={ok:false,checks:[],userAgent:navigator.userAgent};
 try{
  if(await invoke('resource_smoke_restart')){
   const expected=JSON.parse(localStorage.getItem('resource-smoke-restart'));if(!expected)throw Error('Missing restart expectation');Object.assign(result,expected.result);result.ok=false;
   const mirror=new quickgenDurable.TaskMirror(invoke);await mirror.sync();const deadline=Date.now()+20000;
   const terminal=id=>['succeeded','interrupted'].includes(mirror.tasks.get(id)?.state);
   while(expected.pending.some(id=>!terminal(id))&&Date.now()<deadline){await new Promise(r=>setTimeout(r,50));await mirror.sync();}
   if(expected.pending.some(id=>!terminal(id))||expected.pending.filter(id=>mirror.tasks.get(id).state==='interrupted').length>1)throw Error('Queued work did not recover after process restart');
   for(const task of expected.completed)if(JSON.stringify(mirror.tasks.get(task.id))!==JSON.stringify(task))throw Error('Completed task changed after process restart');
   const scene=await invoke('scene_snapshot');if(JSON.stringify(scene)!==JSON.stringify(expected.scene))throw Error('Scene changed after process restart');
   const image=document.getElementById('image');for(const id of expected.pending){const task=mirror.tasks.get(id);if(task.state==='interrupted')continue;const output=await invoke('resource_read',{id:task.outputs[0].resource_id});image.src=output.data;await image.decode();if(image.naturalWidth!==64)throw Error('Restart output unreadable');}
   result.checks.push('actual process exit/restart: queued work recovers, completed attempts remain identical, scene persists and outputs decode');result.restart=true;result.ok=true;await invoke('resource_smoke_finish',{result});return;
  }
  const resume=sessionStorage.getItem('resource-smoke-resume');
  if(resume){
   const saved=JSON.parse(resume);sessionStorage.removeItem('resource-smoke-resume');Object.assign(result,saved.result);
   const mirror=new quickgenDurable.TaskMirror(invoke);await mirror.sync();
   const client=new quickgenDurable.SceneClient(invoke);const scene=await client.load();
   if(scene.items.length!==1||scene.items[0].x!==321||scene.items[0].resourceId!==saved.output)throw Error('Scene lost on reload');
   const deadline=Date.now()+15000;
   while(mirror.tasks.get(saved.taskId)?.state!=='succeeded'&&Date.now()<deadline){await new Promise(r=>setTimeout(r,50));await mirror.sync();}
   if(mirror.tasks.get(saved.taskId)?.state!=='succeeded')throw Error('Host task did not survive WebView reload');
   const prior=mirror.tasks.get(saved.originalId);const duplicate=await client.place(prior,prior.outputs[0],()=>({x:0,y:0}));
   if(duplicate.items.length!==1||duplicate.items[0].x!==321)throw Error('Reload duplicated or moved result');
   result.checks.push('actual WebView reload: host task completes; scene position persists; repeated result placement stays unique');
   await mirror.sync();const completed=[...mirror.tasks.values()];
   const batch=await invoke('task_submit',{submissions:Array.from({length:10},(_,i)=>({submission_id:'restart-'+i,batch_id:'restart-batch',preset_id:'clay',resource_id:saved.output,simulate_failure:false}))});
   const pending=batch.map(r=>r.Ok?.id);if(pending.some(id=>!id))throw Error('Restart batch admission failed');
   const beforeExit=await invoke('task_snapshot');if(!beforeExit.some(t=>pending.includes(t.id)&&t.state==='queued'))throw Error('Restart fixture has no pending work');
   localStorage.setItem('resource-smoke-restart',JSON.stringify({result,scene,completed,pending}));result.ok=true;
   await invoke('resource_smoke_finish',{result});return;
  }
  const id=await invoke('resource_smoke_seed');
  const response=await invoke('resource_read',{id});
  const img=document.getElementById('image');img.src=response.data;await img.decode();
  if(img.naturalWidth!==7||img.naturalHeight!==5)throw Error('Wrong decoded dimensions');
  result.checks.push('managed PNG read through actual IPC and Image.decode: 7x5');
  const paths=await invoke('resource_smoke_paths');
  const native=await invoke('read_image',{path:paths.valid});img.src=native.data;await img.decode();
  if(native.name!=='native-input.png'||img.naturalWidth!==7||img.naturalHeight!==5)throw Error('Native read bridge changed image');
  for(const path of paths.invalid){let rejected=false;try{await invoke('read_image',{path})}catch{rejected=true}if(!rejected)throw Error('Native bridge accepted invalid source');}
  result.checks.push('actual native read_image IPC: bounded worker returns decoded PNG; false extension, damaged and oversized sources rejected');
  for(const bad of ['r-unknown','../secret','https://example.com/image.png']){
   let rejected=false;try{await invoke('resource_read',{id:bad})}catch{rejected=true}
   if(!rejected)throw Error('Unexpectedly accepted '+bad);
   result.checks.push('rejected '+bad);
  }
  const db=await new Promise((resolve,reject)=>{const r=indexedDB.open('quickgen-board-single-v1',1);r.onupgradeneeded=()=>r.result.createObjectStore('scene');r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(r.error)});
  const scene={version:1,items:[{id:'local-1',name:'原图',src:response.data,x:12,y:34,addedAt:123,order:1}],jobs:[],scale:1,tx:2,ty:3,placed:true,pinned:false,open:true,passthrough:false,light:false,nextId:2,jobId:0,selection:['local-1'],selected:'local-1'};
  const first=await quickgenLegacySnapshot.prepare(db,scene,invoke);
  const second=await quickgenLegacySnapshot.prepare(db,scene,invoke);
  if(first.migrationId!==second.migrationId)throw Error('Preparation not idempotent');
  const saved=await new Promise((resolve,reject)=>{const r=db.transaction('scene').objectStore('scene').get('current');r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(r.error)});
  if(JSON.stringify(saved)!==JSON.stringify(scene))throw Error('IndexedDB snapshot mismatch');
  result.migrationId=first.migrationId;result.checks.push('real IndexedDB snapshot preserved; IPC preparation idempotent');db.close();
  const candidate=await invoke('migration_verify',{id:first.migrationId});
  if(candidate.items[0].src!==undefined||candidate.items[0].x!==12||candidate.selection[0]!=='local-1')throw Error('Candidate projection mismatch');
  const migrated=await invoke('resource_read',{id:candidate.items[0].resourceId});img.src=migrated.data;await img.decode();
  const repeated=await invoke('migration_verify',{id:first.migrationId});
  if(repeated.items[0].resourceId!==candidate.items[0].resourceId)throw Error('Repeated verification changed resource');
  result.checks.push('copied and verified candidate through IPC; positions and resource identity preserved');
  const submission={submission_id:'smoke-task-1',batch_id:'smoke-batch',preset_id:'clay',resource_id:candidate.items[0].resourceId,simulate_failure:false};
  const admitted=await invoke('task_submit',{submissions:[submission]});
  const duplicate=await invoke('task_submit',{submissions:[submission]});
  if(!admitted[0].Ok||admitted[0].Ok.id!==duplicate[0].Ok?.id)throw Error('Task admission duplicate');
  const conflict=await invoke('task_submit',{submissions:[{...submission,simulate_failure:true}]});
  if(conflict[0].Err!=='submission_conflict')throw Error('Expected submission conflict');
  await invoke('task_submit',{submissions:[{...submission,submission_id:'smoke-task-failed',simulate_failure:true},{...submission,submission_id:'smoke-task-after-failure'}]});
  let tasks;
  const deadline=Date.now()+15000;
  do {tasks=await invoke('task_snapshot');if(tasks.length===3&&tasks.every(t=>['succeeded','failed'].includes(t.state)))break;await new Promise(r=>setTimeout(r,50));}while(Date.now()<deadline);
  if(tasks.length!==3||tasks[0].state!=='succeeded'||tasks[1].state!=='failed'||tasks[2].state!=='succeeded')throw Error('Serial mock terminal snapshot mismatch: '+JSON.stringify(tasks));
  for(const task of [tasks[0],tasks[2]]){
   const output=await invoke('resource_read',{id:task.outputs[0].resource_id});img.src=output.data;await img.decode();
   if(img.naturalWidth!==64||img.naturalHeight!==64||output.record.display_name!=='模拟结果'||output.record.resource.provenance.task_id!==task.id)throw Error('Mock output mismatch');
  }
  result.checks.push('durable task admission, duplicate reuse, conflict rejection and snapshot through IPC');
  result.checks.push('serial mock success/failure/success; archived outputs decode 64x64 with mock label and task provenance');
  const baseline=await invoke('task_feed');
  const failed=baseline.tasks.find(t=>t.state==='failed');
  const command={task_id:failed.id,attempt:failed.attempt,event_version:failed.event_version,action:'retry'};
  const retried=await invoke('task_control',{command});if(retried.attempt!==2)throw Error('Retry did not create attempt');
  let stale=false;try{await invoke('task_control',{command})}catch(error){stale=String(error).includes('task_version_conflict')}
  if(!stale)throw Error('Stale retry accepted');
  let feed;const feedDeadline=Date.now()+15000;
  do{feed=await invoke('task_feed',{after:baseline.cursor});if(feed.tasks.some(t=>t.id===failed.id&&t.attempt===2&&t.state==='succeeded'))break;await new Promise(r=>setTimeout(r,50));}while(Date.now()<feedDeadline);
  if(!feed.tasks.some(t=>t.id===failed.id&&t.attempt===2&&t.state==='succeeded')||feed.tasks.some(t=>t.id!==failed.id))throw Error('Event replay lost retry or re-executed successful task');
  result.checks.push('versioned retry rejects stale command; durable cursor replays new attempt without rerunning successful tasks');
  const client=new quickgenDurable.SceneClient(invoke);await client.load();
  const placed=await client.place(tasks[0],tasks[0].outputs[0],()=>({x:100,y:200}));
  const moved=await client.mutate(scene=>scene.items[0].x=321);
  let conflictRejected=false;try{await invoke('scene_save',{revision:placed.revision,scene:placed})}catch(error){conflictRejected=String(error).includes('scene_version_conflict')}
  if(!conflictRejected||moved.items[0].x!==321)throw Error('Scene IPC conflict protection failed');
  result.checks.push('scene IPC persists result and movement, rejects stale whole-scene save');
  const reloadTask=await invoke('task_submit',{submissions:[{...submission,submission_id:'smoke-reload-task'}]});
  sessionStorage.setItem('resource-smoke-resume',JSON.stringify({result,output:tasks[0].outputs[0].resource_id,originalId:tasks[0].id,taskId:reloadTask[0].Ok.id}));
  location.reload();return;
 }catch(error){result.error=String(error)}
 await invoke('resource_smoke_finish',{result});
})();
