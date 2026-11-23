const assert=require('node:assert/strict');
const {create}=require('../ui/clipboard-intake.js');
(async()=>{
 let enabled=true, imports=0, messages=[],prevented=0,cleared=0;
 const handle=create({allowed:()=>enabled,importFiles:async files=>{imports++;assert.equal(files[0].type,'image/png');return {added:files.length}},report:x=>messages.push(x),clear:()=>cleared++});
 const event=(files=[{type:'image/png',size:10}],target={})=>({target,clipboardData:{files},preventDefault(){prevented++}});
 enabled=false;await handle(event());assert.equal(prevented,0);enabled=true;
 await handle(event(undefined,{isContentEditable:true}));await handle(event(undefined,{matches:()=>true}));assert.equal(imports,0);assert.equal(prevented,0);
 await handle(event());assert.equal(imports,1);assert.equal(cleared,1);
 await handle(event([]));await handle(event([{type:'text/plain',size:1}]));await handle(event([{type:'image/png',size:21*1024*1024}]));assert.equal(imports,1);assert.equal(messages.length,3);
 const failure=create({allowed:()=>true,importFiles:async()=>{throw Error('private detail')},report:x=>messages.push(x)});await failure(event());assert(!messages.at(-1).includes('private detail'));
 const partial=create({allowed:()=>true,importFiles:async()=>({added:0}),report:x=>messages.push(x),clear:()=>{throw Error('Must not clear on failed import')}});assert.equal(await partial(event()),false);
 console.log('Clipboard intake: mode/editable guards, image intake, empty/unsupported/oversize/read failure passed; native clipboard not tested.');
})().catch(e=>{console.error(e);process.exitCode=1});
