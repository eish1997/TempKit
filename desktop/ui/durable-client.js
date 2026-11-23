'use strict';
// Host-owned work continues independently of these disposable view clients.
(function(root){
 class TaskMirror {
  constructor(invoke){this.invoke=invoke;this.tasks=new Map();this.cursor=null;this.pending=null;this.epoch=0;this.schedulerError=null;this.imageWork=null;}
  apply(task){const old=this.tasks.get(task.id);if(old&&(task.attempt<old.attempt||(task.attempt===old.attempt&&task.event_version<=old.event_version)))return false;this.tasks.set(task.id,structuredClone(task));return true;}
  sync(){
   if(this.pending)return this.pending;
   const epoch=this.epoch;
   const request=this.invoke('task_feed',this.cursor===null?{}:{after:this.cursor}).then(feed=>{
    if(epoch!==this.epoch)return [];
    if(!Number.isSafeInteger(feed.cursor)||feed.cursor<0||(this.cursor!==null&&feed.cursor<this.cursor))throw Error('Invalid task cursor');
    this.schedulerError=feed.scheduler_error||null;
    this.imageWork=feed.image_work?structuredClone(feed.image_work):null;
    const changed=[];for(const task of feed.tasks)if(this.apply(task))changed.push(task);
    this.cursor=feed.cursor;return changed;
   });
   const pending=request.finally(()=>{if(this.pending===pending)this.pending=null});this.pending=pending;return pending;
  }
  reset(){this.epoch++;this.pending=null;this.cursor=null;this.tasks.clear();this.schedulerError=null;this.imageWork=null;}
  async retryImageSave(){const work=this.imageWork;if(!work?.save_failed)throw Error('No image output waiting for save');const next=await this.invoke('image_retry_save',{id:work.task_id,attempt:work.attempt,eventVersion:work.event_version});if(next)this.apply(next);await this.sync();return next;}
  async control(id,action){const task=this.tasks.get(id);if(!task)throw Error('Task unavailable');const next=await this.invoke('task_control',{command:{task_id:id,attempt:task.attempt,event_version:task.event_version,action}});this.apply(next);return next;}
 }
 class SceneClient {
  constructor(invoke){this.invoke=invoke;this.current=null;this.tail=Promise.resolve();}
  enqueue(work){const next=this.tail.then(work);this.tail=next.catch(()=>{});return next;}
  load(){return this.enqueue(async()=>{this.current=await this.invoke('scene_snapshot');return structuredClone(this.current);});}
  // apply is an operation against the latest scene, never a stale whole-scene copy.
  mutate(apply){return this.enqueue(async()=>{
   for(let retry=0;retry<3;retry++){
    if(!this.current)this.current=await this.invoke('scene_snapshot');
    if(!this.current)throw Error('Scene not initialized');
    const scene=structuredClone(this.current);apply(scene);
    try{this.current=await this.invoke('scene_save',{revision:this.current.revision,scene});return structuredClone(this.current)}
    catch(error){if(!String(error).includes('scene_version_conflict'))throw error;this.current=await this.invoke('scene_snapshot');}
   }
   throw Error('Scene changed repeatedly; operation not saved');
  });}
  place(task,output,position){return this.enqueue(async()=>{
   for(let retry=0;retry<3;retry++){
    if(!this.current)this.current=await this.invoke('scene_snapshot');
    if(!this.current)throw Error('Scene not initialized');
    const point=position(structuredClone(this.current));
    try{this.current=await this.invoke('scene_place_output',{placement:{revision:this.current.revision,task_id:task.id,attempt:task.attempt,resource_id:output.resource_id,x:point.x,y:point.y}});return structuredClone(this.current)}
    catch(error){if(!String(error).includes('scene_version_conflict'))throw error;this.current=await this.invoke('scene_snapshot');}
   }
   throw Error('Scene changed repeatedly; output not placed');
  });}
 }
 function sceneDelta(before,after){
  const equal=(a,b)=>JSON.stringify(a)===JSON.stringify(b),changes=[];
  for(const key of Object.keys(after)){
   if(['revision','resourceSchemaVersion','items','jobs'].includes(key))continue;
   if(!equal(before[key],after[key]))changes.push(scene=>{scene[key]=structuredClone(after[key]);});
  }
  for(const key of ['items','jobs']){
   const old=new Map((before[key]||[]).map(item=>[item.id,item])),next=new Map((after[key]||[]).map(item=>[item.id,item]));
   for(const [id] of old)if(!next.has(id))changes.push(scene=>{scene[key]=scene[key].filter(item=>item.id!==id)});
   for(const [id,item] of next){
    if(!old.has(id)){changes.push(scene=>{if(!scene[key].some(row=>row.id===id))scene[key].push(structuredClone(item))});continue;}
    const fields=Object.keys(item).filter(field=>!equal(old.get(id)[field],item[field]));
    if(fields.length)changes.push(scene=>{const row=scene[key].find(row=>row.id===id);if(row)for(const field of fields)row[field]=structuredClone(item[field]);});
   }
  }
  const apply=scene=>{for(const change of changes)change(scene);const ids=new Set(scene.items.map(item=>item.id));scene.selection=scene.selection.filter(id=>ids.has(id));if(scene.selected&&!ids.has(scene.selected))scene.selected=null;};apply.changed=changes.length>0;return apply;
 }
 const api={TaskMirror,SceneClient,sceneDelta};root.quickgenDurable=api;if(typeof module!=='undefined')module.exports=api;
})(globalThis);
