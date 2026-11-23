const assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const source=fs.readFileSync(require('node:path').join(__dirname,'../ui/board.js'),'utf8');const fn=source.slice(source.indexOf('async function copyImage('),source.indexOf("document.addEventListener('copy'"));
(async()=>{
 let clear=0,errors=0,calls=0,release;const context={copyingImage:false,invoke:()=>{calls++;return new Promise(r=>release=r)},hints:{clear:()=>clear++,set:()=>errors++}};vm.createContext(context);vm.runInContext(fn,context);
 const pending=context.copyImage({resourceId:'r-test'});await context.copyImage({resourceId:'r-test'});assert.equal(calls,1);release({copied:true});await pending;assert.equal(clear,1);assert.equal(context.copyingImage,false);
 context.invoke=async()=>({});await context.copyImage({resourceId:'r-test'});assert.equal(errors,1);assert.equal(clear,1);
 context.invoke=async()=>{throw Error('clipboard busy')};await context.copyImage({resourceId:'r-test'});assert.equal(errors,2);assert.equal(context.copyingImage,false);
 console.log('Actual copyImage: duplicate guard, host confirmation required, failure cleanup passed; system clipboard not exercised.');
})().catch(e=>{console.error(e);process.exitCode=1});
