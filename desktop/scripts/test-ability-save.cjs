// IPC contract tests. Native transaction concurrency is tested in storage.rs.
const {test}=require('node:test'),assert=require('node:assert/strict');
const vm=require('node:vm'),fs=require('node:fs'),path=require('node:path');
const source=fs.readFileSync(path.join(__dirname,'../ui/manager.js'),'utf8');
function fixture(){
 let abilities=[],failNext=false;
 const ctx=vm.createContext({invoke:async(cmd,args)=>{
  assert.equal(cmd,'add_ability','the page must not replace a stale ability-list snapshot');
  if(failNext){failNext=false;throw Error('disk full')}
  if(!abilities.some(a=>a.id===args.ability.id))abilities.push(structuredClone(args.ability));
 }});
 vm.runInContext(source.slice(source.indexOf('let abilities='),source.indexOf('function libraryPage()')),ctx);
 return {save:a=>{ctx.next=a;return vm.runInContext('save(next)',ctx)},ids:()=>abilities.map(a=>a.id),fail:()=>failNext=true};
}
test('rapid additions keep both cards instead of overwriting one snapshot',async()=>{
 const f=fixture();await Promise.all([f.save({id:'clay'}),f.save({id:'material'})]);
 assert.deepEqual(f.ids(),['clay','material']);
});
test('a failed write rejects that request without poisoning subsequent additions',async()=>{
 const f=fixture();f.fail();await assert.rejects(f.save({id:'clay'}),/disk full/);
 await f.save({id:'material'});assert.deepEqual(f.ids(),['material']);
});
test('queued duplicate requests still add only once',async()=>{
 const f=fixture();await Promise.all([f.save({id:'clay'}),f.save({id:'clay'})]);
 assert.deepEqual(f.ids(),['clay']);
});
