const assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const source=fs.readFileSync(require('node:path').join(__dirname,'../ui/board.js'),'utf8');
const task={id:'host-only',attempt:1,state:'succeeded',request:{submission_id:'s',preset_id:'clay',inputs:[{resource_id:'original'}]},outputs:[{resource_id:'output'}]};
let placements=0;
const context=vm.createContext({structuredClone,console,Set,Map,Error,JSON,
 hints:{clear(){},set(){}},
 renderedImageWork:'null',
 durable:true,storageReady:true,taskSyncBusy:false,quitting:false,gesture:null,panelLayout:null,
 taskMirror:{sync:async()=>{},tasks:new Map([[task.id,task]])},jobs:[],items:[{id:'input',resourceId:'original',x:0,y:0}],nextId:2,jobId:0,
 persistDurable:async()=>{},sceneClient:{place:async()=>{placements++;return {nextId:3,items:[{id:'result',taskId:task.id,attempt:1,resourceId:'output'}]}}},
 placedTaskOutputs:new Set(),imageSource:async()=> 'image',freeResultPosition:()=>({x:200,y:0}),
 renderImages(){},renderStack(){},renderOutline(){},renderJobs(){},notice(e){throw Error(e);}});
vm.runInContext(source.slice(source.indexOf('async function syncDurableTasks(){'),source.indexOf('const jobs=[];')),context);
(async()=>{await vm.runInContext('syncDurableTasks()',context);assert.equal(placements,1,'accepted host task must place output even when local jobs is empty');assert.equal(context.items.length,2);console.log('host-only output recovery passed');})().catch(e=>{console.error(e);process.exitCode=1});
