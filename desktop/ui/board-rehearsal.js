'use strict';
// Operates only on a copied production profile under --isolated-data.
globalThis.quickgenBoardRehearsal={
 source:null,
 async seed(){
  const db=await new Promise((resolve,reject)=>{const r=indexedDB.open('quickgen-board-single-v1',1);r.onupgradeneeded=()=>r.result.createObjectStore('scene');r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(r.error)});
  this.source=await new Promise((resolve,reject)=>{const r=db.transaction('scene').objectStore('scene').get('current');r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(r.error)});db.close();
 },
 async run(api){
  const invoke=window.__TAURI__.core.invoke,result={ok:false,checks:[]};
  try{
   const status=await invoke('storage_status');if(!status.scene||!api.durable())throw Error('Copied profile did not activate: '+api.restoreError());const scene=status.scene,source=this.source;
   if(!source)throw Error('Copied profile has no legacy scene; inspect the profile mapping');
   if(scene.items.length!==source.items.length||scene.jobs.length!==source.jobs.length)throw Error('Migrated item/job count mismatch');
   for(const old of source.items){const current=scene.items.find(item=>item.id===old.id);if(!current)throw Error('Missing migrated item');for(const key of Object.keys(old))if(key!=='src'&&JSON.stringify(current[key])!==JSON.stringify(old[key]))throw Error('Migrated item field mismatch: '+key);if(!current.resourceId||current.src!==undefined)throw Error('Resource projection missing');}
   for(const key of ['scale','tx','ty','placed','nextId','jobId','pinned','selected','selection','open','passthrough','light'])if(JSON.stringify(scene[key])!==JSON.stringify(source[key]))throw Error('Scene field mismatch: '+key);
   for(let i=0;i<source.jobs.length;i++){const old=source.jobs[i],next=scene.jobs[i];if(next.state!==(['queued','running'].includes(old.state)?'interrupted':old.state))throw Error('Legacy task state mismatch');}
   await Promise.all([...document.querySelectorAll('#world img')].map(img=>img.decode()));
   const copy=await invoke('recovery_export');if(!copy.manifest.files.some(file=>file.path.startsWith('migration-backups/')))throw Error('Recovery export missing legacy backup');
   result.items=scene.items.length;result.historicalJobs=scene.jobs.length;result.recoveryDirectory=copy.directory;
   result.checks.push('copied production profile: all legacy item fields, scene fields and historical states preserved; displayed images decode; recovery bundle exported');
   result.ok=true;
  }catch(error){result.error=String(error);result.notice=document.getElementById('notice')?.textContent;}
  await invoke('board_smoke_finish',{result});
 }
};
