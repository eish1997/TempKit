const assert=require('node:assert/strict');
const {TaskMirror,SceneClient,sceneDelta}=require('../ui/durable-client.js');
(async()=>{
 const before={revision:1,items:[{id:'a',x:1,name:'a'},{id:'deleted',x:2}],jobs:[],selection:['a'],selected:'a'};
 const after=structuredClone(before);after.items[0].name='renamed';after.items.pop();
 const latest=structuredClone(before);latest.revision=2;latest.items[0].x=90;latest.items.push({id:'output',x:30});
 sceneDelta(before,after)(latest);assert.equal(latest.items[0].x,90);assert.equal(latest.items[0].name,'renamed');assert.equal(latest.items[1].id,'output');assert.equal(latest.revision,2);
 let response;
 const mirror=new TaskMirror(async()=>response);
 response={cursor:2,tasks:[{id:'a',attempt:2,event_version:5,state:'queued'}]};await mirror.sync();
 response={cursor:5,tasks:[{id:'a',attempt:1,event_version:9,state:'succeeded'},{id:'a',attempt:2,event_version:4,state:'running'},{id:'a',attempt:2,event_version:6,state:'succeeded'}]};await mirror.sync();assert.equal(mirror.tasks.get('a').state,'succeeded');assert.equal(mirror.tasks.get('a').event_version,6);
 response={cursor:6,tasks:[],scheduler_error:'disk failed'};await mirror.sync();assert.equal(mirror.schedulerError,'disk failed');
 response={cursor:7,tasks:[{id:'archive',attempt:1,event_version:3,state:'finalizing',failure:{stage:'archive',message:'disk full'}}]};await mirror.sync();assert.equal(mirror.tasks.get('archive').failure.message,'disk full');
 response={cursor:8,tasks:[{id:'archive',attempt:1,event_version:4,state:'succeeded'},{id:'archive',attempt:1,event_version:3,state:'finalizing',failure:{stage:'archive',message:'late'}}]};await mirror.sync();assert.equal(mirror.tasks.get('archive').failure,undefined);
 let savedOutput=false;const saveMirror=new TaskMirror(async(command,args)=>{
  if(command==='task_feed')return {cursor:0,tasks:[],image_work:savedOutput?null:{task_id:'pending',attempt:2,event_version:7,save_failed:true}};
  assert.equal(command,'image_retry_save');assert.deepEqual(args,{id:'pending',attempt:2,eventVersion:7});savedOutput=true;return {id:'pending',attempt:2,event_version:8,state:'finalizing'};
 });
 await saveMirror.sync();assert.equal(saveMirror.imageWork.save_failed,true);await saveMirror.retryImageSave();assert.equal(saveMirror.imageWork,null);assert.equal(saveMirror.tasks.get('pending').state,'finalizing');await assert.rejects(saveMirror.retryImageSave(),/No image output/);saveMirror.reset();assert.equal(saveMirror.imageWork,null);
 let release;const stale=new TaskMirror(()=>new Promise(resolve=>release=resolve));const pending=stale.sync();assert.equal(stale.sync(),pending);stale.reset();release({cursor:8,tasks:[{id:'old',attempt:1,event_version:1}]});await pending;assert.equal(stale.tasks.size,0);assert.equal(stale.cursor,null);
 let host={revision:0,items:[{id:'a',x:1}],custom:'keep'},saves=0,fail=false;
 const client=new SceneClient(async(command,args)=>{
  if(command==='scene_snapshot')return structuredClone(host);
  if(command==='scene_save'){
   saves++;if(fail)throw Error('disk full');
   if(saves===1){host.items.push({id:'result',x:20});host.revision++;}
   if(args.revision!==host.revision)throw Error('scene_version_conflict');
   host={...structuredClone(args.scene),revision:host.revision+1};return structuredClone(host);
  }
  throw Error(command);
 });
 await client.load();const saved=await client.mutate(scene=>scene.items.find(i=>i.id==='a').x=100);
 assert.equal(saved.items[0].x,100);assert.equal(saved.items[1].id,'result');assert.equal(saved.custom,'keep');assert.equal(saves,2);
 fail=true;await assert.rejects(client.mutate(scene=>scene.items[0].x=200),/disk full/);assert.equal(client.current.items[0].x,100);
 fail=false;await client.mutate(scene=>scene.items[0].x=300);assert.equal(host.items[0].x,300);
 console.log('durable client: stale events, reset, conflict rebase and failure recovery passed');
})().catch(error=>{console.error(error);process.exitCode=1});
