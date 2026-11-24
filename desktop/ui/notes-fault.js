'use strict';
globalThis.quickgenNotesFault={async run(){
 const invoke=window.__TAURI__.core.invoke,sleep=ms=>new Promise(r=>setTimeout(r,ms)),pending=new Map();
 const result={ok:false,kind:'notes-fault',checks:[],physicalInputTest:false};let token=0,unlisten;
 const require=(condition,message)=>{if(!condition)throw Error(message)};
 const until=async check=>{for(let i=0;i<100;i++){if(await check())return;await sleep(100)}throw Error('Condition timed out')};
 const request=async(command,id,extra={})=>{const n=++token;let timeout;const ack=new Promise((resolve,reject)=>{timeout=setTimeout(()=>{pending.delete(n);reject(Error('Editor acknowledgement timed out'))},10000);pending.set(n,{id,resolve})});try{await invoke(command,{id,token:n,...extra});return await ack}finally{clearTimeout(timeout);pending.delete(n)}};
 const hint=()=>document.getElementById('panel-hint').textContent;
 try{
  require((await invoke('notes_list')).length===0,'Fault test requires fresh isolated store');
  unlisten=await window.__TAURI__.event.listen('note-capacity-ack',e=>{const p=pending.get(e.payload.token);if(p?.id===e.payload.id)p.resolve(e.payload)});
  let note=await invoke('note_create');const blank=await invoke('note_create');
  const rich={version:1,doc:{type:'doc',content:[{type:'blockquote',attrs:{id:'fault-quote'},content:[{type:'paragraph',attrs:{todo:true,done:false},content:[{type:'text',text:'故障前已保存正文',marks:[{type:'strong'}]}]}]}]}};
  note=await invoke('note_patch',{id:note.id,patch:{field:'document',revision:note.text_revision,document:rich}});note=await invoke('note_patch',{id:note.id,patch:{field:'theme',revision:note.theme_revision,theme:'paper'}});result.id=note.id;result.blankId=blank.id;
  await invoke('note_open',{id:blank.id});
  await invoke('notes_test_fault',{id:note.id,stage:'read',enabled:true});await invoke('note_open',{id:note.id});
  await until(()=>hint().includes('isolated note read failure'));
  require((await invoke('note_get',{id:note.id})).text==='故障前已保存正文','Read failure changed stored text');
  await invoke('notes_test_fault',{id:note.id,stage:'read',enabled:false});
  const loaded=await request('notes_test_reload',note.id);require(!loaded.error&&loaded.draft==='故障前已保存正文'&&!loaded.status,'Load retry failed');
  await until(()=>!hint().includes('isolated note read failure'));result.checks.push('Actual editor read failure reaches main hint; retry restores saved text and clears error');
  await invoke('notes_test_fault',{id:note.id,stage:'write',enabled:true});
  const draft='保存失败仍保留的中文草稿',draftDocument=JSON.parse(JSON.stringify(rich));draftDocument.doc.content[0].content[0].content[0].text=draft;const failed=await request('notes_capacity_edit',note.id,{text:draft,document:draftDocument});
  require(failed.error&&failed.draft===draft&&failed.status.includes('保存失败'),'Failed save lost draft or status');
  await until(()=>hint().includes('isolated note write failure'));
  require((await invoke('note_get',{id:note.id})).text==='故障前已保存正文','Failed write replaced durable text');
  require((await invoke('note_get',{id:blank.id})).text==='','Targeted edit leaked to another editor');
  let rejected=false;try{await invoke('notes_flush')}catch(_){rejected=true}finally{await invoke('notes_release')}
  require(rejected,'Flush incorrectly acknowledged unsaved draft');
  await invoke('notes_test_fault',{id:note.id,stage:'write',enabled:false});
  const saved=await request('notes_capacity_edit',note.id,{text:draft,document:draftDocument});require(!saved.error&&saved.draft===draft&&!saved.status,'Save retry failed');
  await until(()=>!hint().includes('isolated note write failure'));
  const durable=await invoke('note_get',{id:note.id});require(durable.text===draft&&durable.theme==='paper'&&durable.document.doc.content[0].content[0].content[0].marks[0].type==='strong'&&durable.document.doc.content[0].content[0].attrs.todo,'Retry lost rich content or theme');require((await invoke('note_get',{id:blank.id})).text==='','Blank note changed');
  result.checks.push('Write failure retains draft and prior durable text; flush rejects; retry saves and clears editor/main errors; blank note unchanged');
  await invoke('notes_flush');const exported=await invoke('recovery_export');result.exportDirectory=exported.directory;result.expectedText=draft;result.ok=true;result.flushExit=true;
 }catch(error){result.error=String(error)}finally{unlisten?.();await invoke('board_smoke_finish',{result})}
}};
