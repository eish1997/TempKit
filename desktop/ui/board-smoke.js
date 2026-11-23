'use strict';
// Loaded only by the diagnostic feature in an explicitly isolated board run.
globalThis.quickgenBoardSmoke={
 async seed(){
  if(localStorage.getItem('board-smoke-seeded'))return;
  const canvas=document.createElement('canvas');canvas.width=24;canvas.height=16;canvas.getContext('2d').fillRect(0,0,24,16);
  const scene={version:1,items:[{id:'local-1',name:'旧现场原图',src:canvas.toDataURL('image/png'),x:40,y:100,addedAt:123,order:1}],jobs:[],scale:1,tx:0,ty:0,placed:true,nextId:2,jobId:0,pinned:true,selected:'local-1',selection:['local-1'],open:true,passthrough:false,light:false};
  const db=await new Promise((resolve,reject)=>{const r=indexedDB.open('quickgen-board-single-v1',1);r.onupgradeneeded=()=>r.result.createObjectStore('scene');r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(r.error)});
  await quickgenLegacySnapshot.persist(db,scene);db.close();localStorage.setItem('board-smoke-source',JSON.stringify(scene));localStorage.setItem('board-smoke-seeded','1');
 },
 async run(api){
  const invoke=window.__TAURI__.core.invoke,result={ok:false,checks:[]};
  const wait=async(test,label)=>{const until=Date.now()+20000;while(!test()){if(Date.now()>until)throw Error('Timeout: '+label);await new Promise(r=>setTimeout(r,100));await api.sync();}};
  try{
   const exitExpected=localStorage.getItem('board-smoke-exit');
   const saved=exitExpected||localStorage.getItem('board-smoke-reload');
   if(saved){
    const expected=JSON.parse(saved);Object.assign(result,expected.result);
    await api.sync();const scene=await invoke('scene_snapshot');
    if(api.items.length!==expected.count||scene.items.length!==expected.count)throw Error('Reload item count mismatch');
    const original=api.items.find(i=>i.id==='local-1');if(original.x!==expected.x||!api.selection.has(original.id))throw Error('Position/selection lost on reload');
    const tasks=await invoke('task_snapshot');if(JSON.stringify(tasks)!==JSON.stringify(expected.tasks))throw Error('Completed tasks changed on reload');
    await Promise.all([...document.querySelectorAll('#world img')].map(img=>img.decode()));
    result.checks.push('real board reload: resource images decode; selection, positions, task attempts and unique outputs persist');
    if(exitExpected){result.checks.push('actual process exit handshake flushes pending keyboard movement; restart restores final position');result.restart=true;result.ok=true;await invoke('board_smoke_finish',{result});return;}
    const element=document.querySelector('[data-image="local-1"].board-image');element.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowRight',bubbles:true}));
    localStorage.setItem('board-smoke-exit',JSON.stringify({...expected,x:original.x,result}));
    result.ok=true;result.flushExit=true;await invoke('board_smoke_finish',{result});return;
   }
   const status=await invoke('storage_status');if(!status.scene||!api.durable())throw Error('Board did not activate');
   const original=api.items.find(i=>i.id==='local-1');if(original.name!=='旧现场原图'||original.x!==40||!original.resourceId)throw Error('Legacy scene changed');
   const old=await new Promise((resolve,reject)=>{const r=indexedDB.open('quickgen-board-single-v1',1);r.onsuccess=()=>{const db=r.result,q=db.transaction('scene').objectStore('scene').get('current');q.onsuccess=()=>{resolve(q.result);db.close()};q.onerror=()=>reject(q.error)};r.onerror=()=>reject(r.error)});
   if(JSON.stringify(old)!==localStorage.getItem('board-smoke-source'))throw Error('Legacy snapshot overwritten');
   result.checks.push('real board activates legacy scene with original position and selection; IndexedDB preserved');
   const canvas=document.createElement('canvas');canvas.width=32;canvas.height=20;canvas.getContext('2d').fillRect(0,0,32,20);
   const blob=await new Promise(resolve=>canvas.toBlob(resolve,'image/png'));
   await api.importFiles([new File([blob],'one.png',{type:'image/png'}),new File(['broken'],'damaged.png',{type:'image/png'}),new File([blob],'two.png',{type:'image/png'})]);
   if(api.items.length!==3)throw Error('Partial import count mismatch');
   document.getElementById('simulate-failure').checked=true;api.simulate(api.items.slice(1),api.cards.get('clay'));
   await wait(()=>api.jobs.length===2&&api.jobs.some(j=>j.state==='failed')&&api.jobs.some(j=>j.state==='done')&&api.items.length===4,'partial execution');
   const failed=api.jobs.find(j=>j.state==='failed');const retry=document.querySelector('[data-job="'+failed.id+'"] button');if(!retry||retry.textContent!=='重试')throw Error('Retry UI missing');retry.click();
   await wait(()=>api.jobs.every(j=>j.state==='done')&&api.items.length===5,'retry');
   const tasks=await invoke('task_snapshot');if(tasks.filter(t=>t.attempt===2).length!==1)throw Error('Retry affected successful task');
   api.simulate([api.items.find(i=>i.result)],api.cards.get('clay'));
   await wait(()=>api.jobs.length===3&&api.jobs.every(j=>j.state==='done')&&api.items.length===6,'result reuse');
   result.checks.push('real board import isolates damaged image; failure retry affects one task; generated result can be submitted again');
   api.select(original.id);const element=document.querySelector('[data-image="local-1"].board-image');element.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowRight',bubbles:true}));
   await api.save();api.hide();api.show(true);await api.save();
   await wait(()=>!api.busy(),'settled saves');
   const expected={result,count:api.items.length,x:original.x,tasks:await invoke('task_snapshot')};localStorage.setItem('board-smoke-reload',JSON.stringify(expected));location.reload();
  }catch(error){result.error=String(error);result.notice=document.getElementById('notice')?.textContent;await invoke('board_smoke_finish',{result});}
 }
};
