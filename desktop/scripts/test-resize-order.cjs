const {test}=require('node:test'),assert=require('node:assert/strict');
const vm=require('node:vm'),fs=require('node:fs'),path=require('node:path');
const app=fs.readFileSync(path.join(__dirname,'../ui/app.js'),'utf8');
test('late native sizing response preserves the latest queued drag size',async()=>{
 const calls=[],pending=[];
 const ctx=vm.createContext({state:{width:280,height:220},configuring:false,queued:false,generation:0,opened:false,
  invoke:(cmd,args)=>{calls.push({...args});return new Promise(resolve=>pending.push(resolve))},
  $:()=>({style:{}}),paint:async()=>{},scheduleSave(){},error:e=>{throw e}});
 vm.runInContext(app.slice(app.indexOf('async function configure()'),app.indexOf('async function toggle(')),ctx);
 const first=vm.runInContext('configure()',ctx);
 vm.runInContext('state.width=420;state.height=310;configure()',ctx);
 pending.shift()({width:280,height:220,entry:{x:0,y:0,width:88,height:18}});await first;
 assert.equal(calls[1].width,420);assert.equal(calls[1].height,310);
 pending.shift()({width:400,height:300,entry:{x:0,y:0,width:88,height:18}});
 for(let i=0;i<8;i++)await Promise.resolve();
 assert.equal(ctx.state.width,400);assert.equal(ctx.state.height,300);
});
