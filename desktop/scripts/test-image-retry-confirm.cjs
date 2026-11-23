const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict');
(async()=>{
 const source=fs.readFileSync('desktop/ui/board.js','utf8'),start=source.indexOf('function confirmRegenerate(task){'),end=source.indexOf('async function syncDurableTasks(){',start);
 let submitted,applied=false,message='';
 const context={trackBoardWork:p=>p,invoke:async(name,args)=>{assert.equal(name,'task_control');submitted=args.command;throw Error('task_version_conflict');},taskMirror:{apply(){applied=true;}},syncDurableTasks:async()=>{},notice:text=>message=text};
 vm.runInNewContext(source.slice(start,end),context);
 await context.confirmRegenerate({id:'task-old',attempt:1,event_version:3});
 assert.deepEqual(JSON.parse(JSON.stringify(submitted)),{task_id:'task-old',attempt:1,event_version:3,action:'regenerate'});assert.equal(applied,false);assert(message.includes('task_version_conflict'));
 console.log('Regeneration confirmation retains the exact attempt/version; stale confirmation cannot retry a newer attempt');
})().catch(e=>{console.error(e);process.exitCode=1});
