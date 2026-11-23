const assert=require('node:assert/strict');
const {mix,ease,createGate}=require('../ui/board-motion.js');
let time=0;const gate=createGate(()=>time);
assert(gate.allows(),'stable open panel allows an intentional drop');
gate.opening();time=5000;assert(!gate.allows(),'waiting during an animation does not bypass protection');
gate.expanded();time=5999;assert(!gate.allows());time=6000;assert(gate.allows());
gate.closed();assert(!gate.allows());gate.opening();time=10000;assert(!gate.allows());gate.expanded();time=11000;assert(gate.allows());
const entry={x:1262,y:300,width:18,height:88},panel={x:1000,y:150,width:280,height:400};
const mid=mix(entry,panel,.4);assert.deepEqual(mix(mid,entry,0),mid,'reversal starts at current shape');assert.deepEqual(mix(mid,entry,1),entry);
let previous=0;for(let i=0;i<=100;i++){const p=ease(i/100);assert(p>=previous&&p>=0&&p<=1);previous=p}
assert(ease(0)<.001&&ease(1)>.999);
console.log('Motion reversal, easing and expansion-complete + 1 second drop gate passed');
// Exercise the actual asynchronous controller, including reversal while a frame is pending.
const {mount}=require('../ui/board-motion.js');
let clock=0,nextFrame=0;const frames=new Map();
global.requestAnimationFrame=fn=>{frames.set(++nextFrame,fn);return nextFrame};global.cancelAnimationFrame=id=>frames.delete(id);
Object.defineProperty(global,'performance',{value:{now:()=>clock},configurable:true});
global.innerWidth=1280;global.innerHeight=720;let reduce=false;global.matchMedia=()=>({get matches(){return reduce}});
function element(rect,hidden=false){const classes=new Set();return{hidden,inert:false,style:{},classList:{add:c=>classes.add(c),remove:c=>classes.delete(c)},getBoundingClientRect:()=>rect}}
let shell;global.document={createElement:()=>shell=element({}),body:{append(){}}};
function advance(ms){clock+=ms;const pending=[...frames.values()];frames.clear();pending.forEach(fn=>fn(clock))}
const watchdog=setTimeout(()=>{console.error('Animation promise did not settle');process.exitCode=1},1000);
(async()=>{
 const dock=element(panel),notch=element(entry,true),controller=mount({dock,notch,prepare:async()=>{}});
 const closing=controller.set(false);await new Promise(setImmediate);advance(100);assert(controller.active);assert(!shell.hidden);const currentClip=shell.style.clipPath;
 const opening=controller.set(true);await new Promise(setImmediate);assert.equal(await closing,false);assert.equal(shell.style.clipPath,currentClip,'reverse does not jump back to the endpoint');advance(241);assert.equal(await opening,true);assert(!dock.hidden&&notch.hidden&&shell.hidden);assert.equal(dock.inert,false);
 reduce=true;assert.equal(await controller.set(false),true);assert(dock.hidden&&!notch.hidden);assert.equal(frames.size,0,'reduced motion schedules no animation');
 console.log('Actual controller reversal and reduced-motion tests passed');
})().catch(error=>{console.error(error);process.exitCode=1}).finally(()=>clearTimeout(watchdog));
