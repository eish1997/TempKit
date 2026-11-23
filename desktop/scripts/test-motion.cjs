// IPC ordering/state tests, not frame-rate or visual animation acceptance.
const {test}=require('node:test');const assert=require('node:assert/strict');
const vm=require('node:vm');const fs=require('node:fs');const path=require('node:path');
const source=fs.readFileSync(path.join(__dirname,'../ui/app.js'),'utf8').split("$('entry').onclick=")[0];
function fixture(){
 const elements=new Map(),calls=[];let release,time=0;
 const document={getElementById:id=>{if(!elements.has(id))elements.set(id,{style:{}});return elements.get(id)},body:{classList:{add(){},remove(){}}}};
 const context=vm.createContext({document,closeManager:()=>Promise.resolve(),quickgenIcons:{},quickgenInput:require('../ui/input-state.js'),matchMedia:()=>({matches:false}),performance:{now:()=>time},setTimeout,clearTimeout,
  requestAnimationFrame:cb=>{time+=280;queueMicrotask(()=>cb(time))},
  window:{__TAURI__:{core:{invoke:(cmd,rect)=>{if(cmd!=='shape')return Promise.resolve();calls.push({...rect});return calls.length===1?new Promise(resolve=>release=resolve):Promise.resolve()}}}}
 });vm.runInContext(source,context);
 vm.runInContext('layout={width:280,height:220,entry:{x:96,y:0,width:88,height:18}}; current={...layout.entry}',context);
 return {run:code=>vm.runInContext(code,context),calls,release:()=>release(),flush:async()=>{for(let i=0;i<5;i++)await Promise.resolve()}};
}
test('reversal waits for native in-flight shape and starts at its applied rectangle',async()=>{
 const f=fixture();const old=f.run('paint({x:60,y:0,width:160,height:100})');await f.flush();
 f.run('opened=true');const closing=f.run('toggle(false)');await f.flush();assert.equal(f.calls.length,1);
 f.release();await old;await closing;
 assert.equal(f.calls[1].width,160);assert.equal(f.calls.at(-1).width,88);
});
test('queued shapes from an invalidated animation never reach native IPC',async()=>{
 const f=fixture();const old=f.run('paint({x:60,y:0,width:160,height:100})');await f.flush();
 const stale=f.run('paint({x:30,y:0,width:210,height:150})');f.run('generation++');f.release();await old;await stale;
 assert.equal(f.calls.length,1);
});
