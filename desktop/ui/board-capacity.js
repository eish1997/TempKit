'use strict';
globalThis.quickgenBoardCapacity={
 async seed(){
  if(localStorage.getItem('capacity-seeded'))return;
  const items=[];let bytes=0;const canvas=document.createElement('canvas');canvas.width=512;canvas.height=512;const ctx=canvas.getContext('2d');
  for(let n=0;n<100;n++){
   for(let y=0;y<16;y++)for(let x=0;x<16;x++){ctx.fillStyle=`rgb(${(n*31+x*17)%256},${(n*19+y*23)%256},${(n*13+x*y*7)%256})`;ctx.fillRect(x*32,y*32,32,32);}
   const src=canvas.toDataURL('image/png');bytes+=atob(src.split(',')[1]).length;
   items.push({id:'local-'+(n+1),name:'容量图 '+(n+1),src,x:40+(n%5)*198,y:100+Math.floor(n/5)*160,addedAt:n,order:n+1});
  }
  const scene={version:1,items,jobs:[],scale:.5,tx:0,ty:0,placed:true,nextId:101,jobId:0,pinned:false,selected:null,selection:[],open:false,passthrough:false,light:false};
  const db=await new Promise((resolve,reject)=>{const r=indexedDB.open('quickgen-board-single-v1',1);r.onupgradeneeded=()=>r.result.createObjectStore('scene');r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(r.error)});
  await quickgenLegacySnapshot.persist(db,scene);db.close();localStorage.setItem('capacity-sample',JSON.stringify({images:100,width:512,height:512,encodedBytes:bytes}));localStorage.setItem('capacity-seeded','1');
 },
 async run(api){
  const invoke=window.__TAURI__.core.invoke,start=performance.now(),result={ok:false,sample:JSON.parse(localStorage.getItem('capacity-sample')),checks:[],switches:0,lagSamples:0,lagOver100ms:0,maxLagMs:0};
  let last=performance.now();const timer=setInterval(()=>{const now=performance.now(),lag=Math.max(0,now-last-250);last=now;result.lagSamples++;result.maxLagMs=Math.max(result.maxLagMs,lag);if(lag>100)result.lagOver100ms++;},250);
  try{
   if(api.items.length!==100)throw Error('Capacity seed mismatch');
   api.simulate(api.items.slice(0,50),api.cards.get('clay'));
   const deadline=Date.now()+180000;
   while(api.jobs.filter(j=>j.state==='done').length!==50||api.items.length!==150){if(Date.now()>deadline)throw Error('50 task completion timeout');await new Promise(r=>setTimeout(r,250));await api.sync();}
   result.processingMs=performance.now()-start;await api.save();
   result.checks.push('100 managed 512x512 originals and 50 mock outputs present; all 50 tasks complete');
   api.show(true);api.pass(true,true);await api.save();
   let nextSwitch=0,nextReport=0;
   while(performance.now()-start<30*60*1000){
    const elapsed=performance.now()-start;
    if(result.switches<20&&elapsed>=nextSwitch){api.hide();api.show(true);api.pass(true,true);result.switches++;await api.save();nextSwitch=elapsed+30000;}
    if(elapsed>=nextReport){result.elapsedMs=elapsed;result.items=api.items.length;result.jobs=api.jobs.length;await invoke('board_capacity_progress',{result});nextReport=elapsed+10000;}
    await new Promise(r=>setTimeout(r,500));
   }
   await api.sync();await api.save();const scene=await invoke('scene_snapshot'),tasks=await invoke('task_snapshot');
   if(scene.items.length!==150||tasks.length!==50||tasks.some(t=>t.state!=='succeeded'||t.attempt!==1)||result.switches!==20)throw Error('Capacity final state mismatch');
   result.elapsedMs=performance.now()-start;result.checks.push('30 minute run and 20 board close/open cycles complete; no duplicate tasks or results');result.ok=true;
  }catch(error){result.error=String(error);}finally{clearInterval(timer);await invoke('board_smoke_finish',{result});}
 }
};
