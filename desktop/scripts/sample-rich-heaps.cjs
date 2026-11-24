// Read-only CDP metrics for a specifically identified isolated diagnostic child.
// No debugger/profiler is enabled, no expressions are evaluated, no GC is forced.
const fs=require('node:fs'),path=require('node:path');
const base=path.resolve(__dirname,'../evidence/notes-rich-migration');
const identityPath=path.resolve(process.argv[2]||'');
if(!identityPath.startsWith(base+path.sep))throw Error('Identity must be within rich migration evidence');
const identity=JSON.parse(fs.readFileSync(identityPath,'utf8').replace(/^\uFEFF/,''));
if(!identity.diagnosticPort||identity.mode!=='notes-rich-capacity'||!identity.desktop.startsWith('QuickGenTest-'))throw Error('Not an owned capacity diagnostic');
const out=identityPath.replace(/\.identity\.json$/,'.heaps.jsonl'),started=Date.now();
const delay=ms=>new Promise(r=>setTimeout(r,ms));
async function measure(target){
 const ws=new WebSocket(target.webSocketDebuggerUrl);let next=0;const pending=new Map();
 try{
  await new Promise((resolve,reject)=>{const timer=setTimeout(()=>reject(Error('CDP open timed out: '+target.url)),5000);ws.onopen=()=>{clearTimeout(timer);resolve();};ws.onerror=e=>{clearTimeout(timer);reject(Error('CDP open: '+target.url+' '+e.message));};});
  ws.onmessage=e=>{const m=JSON.parse(e.data),p=pending.get(m.id);if(p){pending.delete(m.id);m.error?p.reject(Error(JSON.stringify(m.error))):p.resolve(m.result);}};
  const send=method=>new Promise((resolve,reject)=>{const id=++next,timer=setTimeout(()=>{pending.delete(id);reject(Error('CDP response timed out: '+method));},5000);pending.set(id,{resolve:r=>{clearTimeout(timer);resolve(r);},reject:e=>{clearTimeout(timer);reject(e);}});ws.send(JSON.stringify({id,method}));});
  return {id:target.id,url:target.url,heap:await send('Runtime.getHeapUsage'),dom:await send('Memory.getDOMCounters')};
 }finally{ws.close();}
}
(async()=>{
 while(Date.now()-started<1920000){
  const now=JSON.parse(fs.readFileSync(identityPath,'utf8').replace(/^\uFEFF/,''));if(now.status==='exited')break;
  try{
   const targets=await(await fetch(`http://127.0.0.1:${identity.diagnosticPort}/json/list`,{signal:AbortSignal.timeout(5000)})).json();
   const pages=targets.filter(t=>t.type==='page'&&/^http:\/\/tauri\.localhost\/(note|ink|board)\.html/.test(t.url));
   if(pages.filter(t=>t.url.includes('/note.html?id=note-'+identity.pid+'-')).length!==10)throw Error('Expected ten notes owned by this diagnostic PID');
   const rows=[];for(const page of pages)rows.push(await measure(page));const sample={utc:new Date().toISOString(),elapsedSeconds:(Date.now()-Date.parse(identity.startedUtc))/1000,pages:rows};
   fs.appendFileSync(out,JSON.stringify(sample)+'\n');console.log('Heap sample '+Math.round(sample.elapsedSeconds)+'s pages='+rows.length);
  }catch(error){fs.appendFileSync(out,JSON.stringify({utc:new Date().toISOString(),error:String(error)})+'\n');if(Date.now()-started>1800000)break;}
  await delay(60000);
 }
})().catch(e=>{console.error(e);process.exitCode=1});
