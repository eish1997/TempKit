// Runs the actual app handlers with fake native IPC and image decoding.
// Proves asynchronous decision ordering, not Windows drag/drop or image compatibility.
const {test}=require('node:test');const assert=require('node:assert/strict');
const vm=require('node:vm'),fs=require('node:fs'),path=require('node:path');
const input=require('../ui/input-state.js');
const source=fs.readFileSync(path.join(__dirname,'../ui/app.js'),'utf8');
async function fixture(recoveryWarning){
 let now=0,channel;const records=[],decodes=[],elements=new Map();
 function element(){return {style:{setProperty(){}},dataset:{},hidden:false,textContent:'',classList:{add(){},remove(){}},setAttribute(){},removeAttribute(){},append(){},replaceChildren(){},addEventListener(){},querySelector:()=>({style:{}})}}
 const card=element();card.textContent='测试能力';card.closest=()=>card;
 const document={getElementById:id=>{if(!elements.has(id))elements.set(id,element());return elements.get(id)},documentElement:element(),body:element(),createElement:element,querySelectorAll:()=>[],addEventListener(){},elementFromPoint:()=>card};
 const invoke=async(cmd,args)=>{
  if(cmd==='load_config')return {layout:{edge:'top',fraction:.5,width:280,height:220,heightIndex:2},abilities:[],recoveryWarning};
  if(cmd==='configure')return {width:280,height:220,entry:{x:96,y:0,width:88,height:18},area:{x:0,y:0,width:1920,height:1080}};
  if(cmd==='read_image'){
   if(!input.acceptsPath(args.path))throw Error('请拖入 PNG、JPG 或 WebP 图片');
   return {path:args.path,data:'data:image/png;base64,fixture'};
  }
  if(cmd==='record_trigger')records.push(args);
 };
 const context=vm.createContext({createDisplayWatch:require('../ui/display-watch.js'),setInterval(){},document,quickgenIcons:{},quickgenInput:{...input,createCardDropGate:()=>input.createCardDropGate(()=>now)},
  matchMedia:()=>({matches:false}),performance:{now:()=>now},setTimeout:()=>0,clearTimeout(){},cancelAnimationFrame(){},
  requestAnimationFrame:fn=>{now+=280;queueMicrotask(()=>fn(now));return 1},DOMException,
  Image:class{decode(){return new Promise((resolve,reject)=>{resolve.reject=reject;decodes.push(resolve)})}},
  window:{devicePixelRatio:1,__TAURI__:{core:{invoke,Channel:class{constructor(){channel=this}}},event:{listen:async()=>{}},window:{getCurrentWindow:()=>({onCloseRequested:async()=>{}})}}}
 });vm.runInContext(source,context);
 const flush=async()=>{for(let i=0;i<30;i++)await Promise.resolve()};await flush();
 return {records,decodes,elements,flush,advance:ms=>now+=ms,run:code=>vm.runInContext(code,context),
  enter:paths=>channel.onmessage({type:'enter',paths,position:{x:140,y:9}}),
  drop:(paths=['fixture.png'])=>channel.onmessage({type:'drop',paths,position:{x:40,y:80}})};
}
test('configuration recovery opens the native panel region before showing its warning',async()=>{
 const warning='配置损坏，已保留原文件并恢复默认';const f=await fixture(warning);await f.flush();
 assert.equal(f.run('opened'),true);assert.equal(f.run('current.height'),220);
 assert.equal(f.elements.get('error').textContent,warning);
 const normal=await fixture();assert.equal(normal.run('opened'),false);
});
test('unknown hover paths can arm expansion, but known unsupported and multiple files cannot',async()=>{
 for(const paths of [[],['']]){
  const f=await fixture();await f.enter(paths);assert.equal(f.run('entryDrag!==null'),true);
  assert.equal(f.records.length,0);
 }
 for(const paths of [['text.txt'],['one.png','two.png']]){
  const f=await fixture();await f.enter(paths);assert.equal(f.run('entryDrag'),null);
 }
});

test('early release stays buffered even when decoding completes after the one-second deadline',async()=>{
 const f=await fixture();await f.run('toggle(true)');f.advance(999);
 const drop=f.drop();await f.flush();f.advance(2000);f.decodes.shift()();await drop;
 assert.equal(f.records.length,0);assert.equal(f.elements.get('pending').hidden,false);
 await f.run("trigger({textContent:'测试能力'})");assert.equal(f.records.length,1);assert.equal(f.elements.get('pending').hidden,true);
});
test('mature panel drop triggers once and clears the buffered image',async()=>{
 const f=await fixture();await f.run('toggle(true)');f.advance(1000);
 const drop=f.drop();await f.flush();f.decodes.shift()();await drop;
 assert.equal(f.records.length,1);assert.equal(f.elements.get('pending').hidden,true);
 await f.run("trigger({textContent:'测试能力'})");assert.equal(f.records.length,1);
});
test('collapse and reopen require a new grace period',async()=>{
 const f=await fixture();await f.run('toggle(true)');f.advance(2000);await f.run('toggle(false)');await f.run('toggle(true)');
 const drop=f.drop();await f.flush();f.decodes.shift()();await drop;assert.equal(f.records.length,0);
});
test('clear button removes buffered image and prevents subsequent card clicks from triggering',async()=>{
 const f=await fixture();await f.run('toggle(true)');const drop=f.drop();await f.flush();f.decodes.shift()();await drop;
 f.elements.get('pending').onclick();assert.equal(f.elements.get('pending').hidden,true);
 await f.run("trigger({textContent:'测试能力'})");assert.equal(f.records.length,0);
});
test('clearing while a replacement decodes does not let the late image restore the buffer',async()=>{
 const f=await fixture();await f.run('toggle(true)');let drop=f.drop();await f.flush();f.decodes.shift()();await drop;
 drop=f.drop();await f.flush();f.elements.get('pending').onclick();f.decodes.shift()();await drop;
 assert.equal(f.elements.get('pending').hidden,true);assert.equal(f.records.length,0);
 // A new user drop after clearing can still populate the buffer.
 drop=f.drop();await f.flush();f.decodes.shift()();await drop;assert.equal(f.elements.get('pending').hidden,false);
});

for(const invalid of ['unsupported','multiple','corrupt'])test(`${invalid} drop preserves the existing buffer and does not trigger`,async()=>{
 const f=await fixture();await f.run('toggle(true)');
 let work=f.drop(['original.png']);await f.flush();f.decodes.shift()();await work;
 const original=f.elements.get('pending-preview').src;f.advance(1500);
 if(invalid==='unsupported')await f.drop(['unsupported.txt']);
 if(invalid==='multiple')await f.drop(['one.png','two.png']);
 if(invalid==='corrupt'){
  work=f.drop(['broken.png']);await f.flush();f.decodes.shift().reject(new DOMException('decode failed'));await work;
 }
 assert.equal(f.elements.get('pending').hidden,false);
 assert.equal(f.elements.get('pending-preview').src,original);
 assert.notEqual(f.elements.get('error').textContent,'');assert.equal(f.records.length,0);
 // It remains the original usable input, not merely an unchanged thumbnail.
 await f.run("trigger({textContent:'测试能力'})");
 assert.equal(f.records.length,1);assert.equal(f.records[0].path,'original.png');
});
