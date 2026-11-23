const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const source = fs.readFileSync(require('node:path').join(__dirname, '../ui/board.js'), 'utf8');
const start = source.indexOf('async function initStorage(){');
const end = source.indexOf('function saveSoon()', start);
assert(start >= 0 && end > start);
const implementation = source.slice(start, end);
const scene = () => ({version:1,resourceSchemaVersion:1,revision:0,items:[],jobs:[],selection:[],selected:null,scale:1,tx:0,ty:0,placed:true,nextId:1,jobId:0,pinned:false,open:false,passthrough:false,light:false});

async function run({active=null,allowed=false,legacy=null,migrationError=false}) {
  const calls=[],modes=[];let opens=0,closes=0;
  const database = {
    transaction(store,mode) {assert.equal(store,'scene');modes.push(mode);return {objectStore(){return {get(key){assert.equal(key,'current');const request={};queueMicrotask(()=>{request.result=legacy;request.onsuccess();});return request;}};}};},
    close(){closes++;},
  };
  const context = vm.createContext({
    structuredClone,console:{error(){}},
    document:{body:{inert:false,classList:{toggle(){}}}},
    indexedDB:{open(){opens++;const request={};queueMicrotask(()=>{request.result=database;request.onsuccess();});return request;}},
    invoke:async(name,args)=>{calls.push({name,args});if(name==='storage_status')return {scene:active,activationAllowed:allowed};if(name==='migration_activate'){if(migrationError)throw Error('injected migration failure');return scene();}throw Error('Unexpected command '+name);},
    $:()=>({setAttribute(){}}),notice(){},
    renderStack(){},renderImages(){},renderOutline(){},camera(){},showCanvas(){},setPassThrough(){},
    imageSource:async()=>{throw Error('Fixture should not read images');},
    taskMirror:{sync:async()=>{}},submitDurable:async()=>{throw Error('Fixture should not submit');},syncDurableTasks(){},setInterval(){},
    snapshotScene:()=>scene(),
  });
  vm.runInContext('let storageReady=false,storageError=null,durable=false,lastLocalScene=null;const sceneClient={},items=[],jobs=[],selection=new Set();let scale,tx,ty,placed,nextId,jobId,pinned,selected;'+implementation,context);
  await vm.runInContext('initStorage()',context);
  return {opens,closes,modes,calls,state:vm.runInContext('({storageReady,storageError,durable})',context)};
}
(async()=>{
  const blocked=await run({});
  assert.equal(blocked.opens,0,'Closed migration gate must not touch IndexedDB');
  assert.equal(blocked.state.storageReady,false);assert(blocked.state.storageError);
  assert.deepEqual(blocked.calls.map(c=>c.name),['storage_status']);

  const active=await run({active:scene()});
  assert.equal(active.opens,0,'Activated scene must bypass legacy storage');
  assert.equal(active.state.storageReady,true);assert.equal(active.state.durable,true);

  const old={...scene(),resourceSchemaVersion:undefined,items:[{id:'legacy',src:'original body',x:17}]};
  const original=structuredClone(old);
  const migrated=await run({allowed:true,legacy:old});
  assert.equal(migrated.closes,1);assert.deepEqual(migrated.modes,[undefined],'Legacy transaction must remain readonly');
  assert.deepEqual(migrated.calls.find(c=>c.name==='migration_activate').args.snapshot,original);
  assert.deepEqual(old,original,'Migration must not mutate the legacy source');assert.equal(migrated.state.storageReady,true);

  const failed=await run({allowed:true,legacy:old,migrationError:true});
  assert.equal(failed.closes,1);assert.equal(failed.state.storageReady,false);assert.match(failed.state.storageError,/injected/);
  assert.deepEqual(failed.modes,[undefined]);assert.deepEqual(old,original);

  const future=await run({active:{...scene(),resourceSchemaVersion:2}});
  assert.equal(future.opens,0);assert.equal(future.state.storageReady,false);
  for(const [name,mode] of [['boardCapacity','reliable-capacity'],['boardRehearsal','reliable-rehearsal'],['boardSmoke','reliable-base'],['boardReview','reliable-review']]){
    const declaration=source.split('\n').find(line=>line.startsWith('const '+name+'='));assert(declaration);
    for(const isolated of [false,true]){
      const context=vm.createContext({diagnostic:mode,invoke:async()=>({activationAllowed:true,isolated})});
      assert.equal(await vm.runInContext('(async()=>{'+declaration+'return '+name+';})()',context),isolated,'Diagnostic fixtures must require isolation even when production migration is enabled');
    }
  }
  console.log('Storage authority: closed gate, active-store bypass, readonly legacy migration, failure preservation and future-version refusal passed');
})().catch(error=>{console.error(error);process.exitCode=1;});
