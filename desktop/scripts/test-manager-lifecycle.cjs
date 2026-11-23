// Actual app controller with delayed IPC; not native window acceptance.
const {test}=require('node:test');
const assert=require('node:assert/strict');
const vm=require('node:vm'),fs=require('node:fs'),path=require('node:path');
const app=fs.readFileSync(path.join(__dirname,'../ui/app.js'),'utf8');
const source=app.slice(app.indexOf('let openingManager='),app.indexOf("$('add').onclick="));
function fixture(){
 const calls=[],errors=[];let finish;
 const context=vm.createContext({opened:true,error:e=>errors.push(String(e)),invoke:(cmd,args)=>{
  calls.push(cmd);return cmd==='open_manager'?new Promise((resolve,reject)=>finish={resolve,reject}):Promise.resolve();
 }});
 vm.runInContext(source,context);
 return {calls,errors,run:s=>vm.runInContext(s,context),finish:()=>finish};
}
test('closing during creation prevents a late child from appearing',async()=>{
 const f=fixture(),opening=f.run("openManager('add')");
 f.run('opened=false;closeManager()');f.finish().resolve();await opening;
 assert.equal(f.calls.includes('show_manager'),false);
});
test('collapse then reopen does not revive an obsolete request; a fresh request works',async()=>{
 const f=fixture(),old=f.run("openManager('library')");
 f.run('opened=false;closeManager();opened=true');f.finish().resolve();await old;
 assert.equal(f.calls.includes('show_manager'),false);
 const next=f.run("openManager('add')");f.finish().resolve();await next;
 assert.equal(f.calls.filter(x=>x==='show_manager').length,1);
});
test('a failed creation releases the guard and permits retry',async()=>{
 const f=fixture(),old=f.run("openManager('add')");f.finish().reject('creation failed');await old;
 assert.deepEqual(f.errors,['creation failed']);
 const next=f.run("openManager('add')");f.finish().resolve();await next;
 assert.equal(f.calls.filter(x=>x==='show_manager').length,1);
});
