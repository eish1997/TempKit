const {test}=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs'),vm=require('node:vm'),path=require('node:path');
function setup(invoke){
 const roots=[],content={replaceChildren(root){this.root=root;}},heading={},back={},message={};
 const document={activeElement:null,createElement(){const root={secrets:[],querySelectorAll(){return this.secrets;}};roots.push(root);return root;},getElementById(){return null;}};
 const context={window:{},document,Error};vm.runInNewContext(fs.readFileSync(path.join(__dirname,'../ui/manager-navigation.js'),'utf8'),context);
 return {nav:context.window.quickgenManagerNavigation.create({content,heading,back,message,invoke:(command,args)=>command==='manager_page_size'?Promise.resolve():invoke(command,args)}),content,heading,back,message,roots};
}
test('late request cannot write a new page or issue follow-up host work',async()=>{
 let resolve,started;const entered=new Promise(r=>started=r);const calls=[];const f=setup(command=>{calls.push(command);started();return new Promise(r=>resolve=r);});
 const old=f.nav.start({title:'连接',mount:async ctx=>{await ctx.invoke('probe');await ctx.invoke('revoke');ctx.root.result='old';}});
 await entered;
 await f.nav.push({title:'设置',mount:async ctx=>{ctx.root.result='new';}});
 resolve({});await old;
 assert.deepEqual(calls,['probe']);assert.equal(f.content.root.result,'new');assert.equal(f.message.textContent,'');assert.equal(f.heading.textContent,'设置');
});
test('leaving clears passwords, runs cleanup, and back remounts source',async()=>{
 const f=setup(async()=>({}));let mounts=0,cleanups=0;const secret={value:'synthetic-token'};
 await f.nav.start({title:'来源',mount:async ctx=>{mounts++;ctx.root.secrets.push(secret);ctx.onDispose(()=>cleanups++);}});
 await f.nav.push({title:'详情',mount:async()=>{}});
 assert.equal(secret.value,'');assert.equal(cleanups,1);assert.equal(f.back.hidden,false);
 await f.nav.back();assert.equal(mounts,2);assert.equal(f.heading.textContent,'来源');assert.equal(f.back.hidden,true);
});
test('current errors remain visible while expired errors are ignored',async()=>{
 const f=setup(async()=>{throw Error('offline');});
 await f.nav.start({title:'连接',mount:async ctx=>{await ctx.invoke('probe');}});
 assert.match(f.message.textContent,/offline/);
});
