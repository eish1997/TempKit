'use strict';
globalThis.quickgenNotesRestart={async run(api){
 const invoke=window.__TAURI__.core.invoke,sleep=ms=>new Promise(r=>setTimeout(r,ms));
 const result={ok:false,kind:'notes-restart',physicalInputTest:false,editors:[]};let unlisten,token=0;const pending=new Map();
 try{
  const before=await invoke('notes_list');if(!before.length)throw Error('Restart test requires populated isolated store');
  result.before=before;
  unlisten=await window.__TAURI__.event.listen('note-capacity-ack',e=>{const p=pending.get(e.payload.token);if(p?.id===e.payload.id)p.resolve(e.payload)});
  let ready=false;for(let i=0;i<3&&!ready;i++){try{await invoke('notes_flush');ready=true}catch(error){if(i===2)throw error}finally{await invoke('notes_release')}if(!ready)await sleep(300)}
  const resident=before.filter(n=>n.resident&&!n.deleted),windows=await invoke('notes_capacity_windows');
  if(windows.total!==resident.length||windows.visible!==resident.length)throw Error('Restart created incorrect resident windows');
  result.transitionCycles=0;
  const checkVisible=async(expected,stage)=>{const first=await invoke('notes_capacity_windows');if(first.visible===expected)return;result.windowTransitionSamples=[{afterMs:0,...first}];for(let i=1;i<=5;i++){await sleep(100);result.windowTransitionSamples.push({afterMs:i*100,...await invoke('notes_capacity_windows')})}throw Error(stage+' visibility mismatch')};
  for(let cycle=0;cycle<60;cycle++){await api.set(true);await checkVisible(0,'overview');
   if(cycle===0&&resident.length){const id=resident[0].id,n=++token;let timeout;const ack=new Promise((resolve,reject)=>{timeout=setTimeout(()=>reject(Error('Late ready acknowledgement timed out')),10000);pending.set(n,{id,resolve})});try{await invoke('notes_test_reload',{id,token:n});const editor=await ack;if(editor.error)throw Error(editor.error);await checkVisible(0,'late editor ready in overview');result.lateReadyStayedHidden=true}finally{clearTimeout(timeout);pending.delete(n)}}
   await sleep(100);await api.set(false);await checkVisible(resident.length,'restore');result.transitionCycles++;}
  for(const note of resident){const n=++token;let timeout;const ack=new Promise((resolve,reject)=>{timeout=setTimeout(()=>reject(Error('Restart editor acknowledgement timed out')),10000);pending.set(n,{id:note.id,resolve})});
   try{await invoke('notes_test_reload',{id:note.id,token:n});const editor=await ack;if(editor.error||editor.draft!==note.text||editor.status)throw Error('Restart editor text mismatch');result.editors.push({id:note.id,text:editor.draft})}finally{clearTimeout(timeout);pending.delete(n)}
  }
  await invoke('notes_flush');result.after=await invoke('notes_list');
  const durable=notes=>JSON.stringify(notes.map(({id,text,document,theme,text_revision,theme_revision,created_at,resident,always_on_top,deleted})=>({id,text,document,theme,text_revision,theme_revision,created_at,resident,always_on_top,deleted})).sort((a,b)=>a.id.localeCompare(b.id)));
  if(durable(before)!==durable(result.after))throw Error('Restart changed durable note document, theme, versions or flags');
  result.windows=windows;result.ok=true;result.flushExit=true;
 }catch(error){result.error=String(error)}finally{unlisten?.();await invoke('board_smoke_finish',{result})}
}};
