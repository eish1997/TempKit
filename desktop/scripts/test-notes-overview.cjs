const assert=require('node:assert/strict'),{zoomAt,fit}=require('../ui/notes-overview.js');
const c={x:-30,y:50,scale:.75},x=320,y=280,n=zoomAt(c,x,y,-220);
assert(Math.abs((x-c.x)/c.scale-(x-n.x)/n.scale)<1e-9);assert(Math.abs((y-c.y)/c.scale-(y-n.y)/n.scale)<1e-9);
assert.equal(zoomAt(c,x,y,1e8).scale,.12);assert.equal(zoomAt(c,x,y,-1e8).scale,3);
for(const count of [1,6,100]){const v=fit(count,1200,800);assert(v.columns>0);assert(v.scale>=.12&&v.scale<=1);assert(Number.isFinite(v.x)&&Number.isFinite(v.y));}
console.log('Overview pointer anchor, zoom bounds and finite initial layout passed; native gestures not covered.');
const {createMode}=require('../ui/notes-overview.js');
(async()=>{
 let notes=[{id:'a',deleted:false,resident:false}],failSave=false,failRestore=false,visible=true,releases=0,empty=0,active=false;
 const mode=createMode({list:async()=>notes,flush:async()=>{if(failSave)throw Error('save failed')},present:async value=>{if(!value&&failRestore)throw Error('restore failed');visible=!value},release:async()=>releases++,render:n=>assert.equal(n.length,1),changed:v=>active=v,empty:()=>empty++});
 failSave=true;await assert.rejects(mode.set(true),/save failed/);assert.equal(mode.active,false);assert.equal(visible,true);
 failSave=false;await mode.set(true);assert.equal(mode.active,true,'all nonresident still enters');assert.equal(visible,false);
 failRestore=true;await assert.rejects(mode.set(false),/restore failed/);assert.equal(mode.active,true,'failed exit stays retryable');
 failRestore=false;await mode.set(false);assert.equal(mode.active,false);assert.equal(visible,true);
 await Promise.all([mode.set(true),mode.set(false),mode.set(true)]);assert.equal(active,true);await mode.set(false);
 notes=[];await mode.set(true);assert.equal(mode.active,false);assert.equal(empty,1);assert(releases>=3);
 console.log('Overview lifecycle: failed save, all nonresident, exit failure/retry, rapid transitions and zero notes passed.');
})().catch(e=>{console.error(e);process.exitCode=1});
