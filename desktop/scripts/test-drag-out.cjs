const assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const source=fs.readFileSync(require('node:path').join(__dirname,'../ui/board.js'),'utf8');
const fn=source.slice(source.indexOf('async function dragOut('),source.indexOf('function startDrag('));
(async()=>{
 let release, calls=0, syncs=0,clears=0,errors=0;
 const context={outboundDragging:false,notesMode:{active:false},passthrough:false,quitting:false,cancelGesture(){},invoke:()=>{calls++;return new Promise(r=>release=r)},hints:{clear:()=>clears++,set:()=>errors++},syncNativeSoon:()=>syncs++};
 vm.createContext(context);vm.runInContext(fn,context);
 const pending=context.dragOut({resourceId:'r-test'});assert.equal(context.outboundDragging,true);
 await context.dragOut({resourceId:'r-test'});assert.equal(calls,1);
 release({outcome:'Cancelled'});await pending;assert.equal(context.outboundDragging,false);assert.equal(syncs,1);assert.equal(clears,1);
 context.invoke=async()=>{throw Error('failed')};await context.dragOut({resourceId:'r-test'});assert.equal(errors,1);assert.equal(syncs,2);assert.equal(context.outboundDragging,false);
 for(const field of ['passthrough','quitting']){context[field]=true;await context.dragOut({resourceId:'r-test'});context[field]=false;}context.notesMode.active=true;await context.dragOut({resourceId:'r-test'});assert.equal(syncs,2);
 console.log('Actual dragOut function: duplicate guard, cancel/error cleanup and mode guards passed; OLE not exercised.');
})().catch(e=>{console.error(e);process.exitCode=1});
