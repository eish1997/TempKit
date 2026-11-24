// Independent browser diagnostic: explicit GC is confined to this disposable
// fixture. It never runs against the capacity test or a user's browser.
const {spawn}=require('node:child_process'),fs=require('node:fs'),path=require('node:path');
const dir=path.resolve(__dirname,'../evidence/notes-rich-migration'),profile=path.join(dir,'retention-'+Date.now()),delay=ms=>new Promise(r=>setTimeout(r,ms));
(async()=>{
 const child=spawn('C:/Program Files/Google/Chrome/Application/chrome.exe',['--headless=new','--disable-gpu','--no-first-run','--no-default-browser-check','--remote-debugging-port=0','--user-data-dir='+profile,'about:blank'],{windowsHide:true,stdio:'ignore'});let ws;
 try{
  let port;for(let i=0;i<100;i++){try{port=fs.readFileSync(path.join(profile,'DevToolsActivePort'),'utf8').split('\n')[0];break;}catch{}await delay(100);}if(!port)throw Error('Disposable browser did not start');
  const pages=await(await fetch(`http://127.0.0.1:${port}/json/list`)).json();ws=new WebSocket(pages.find(p=>p.type==='page').webSocketDebuggerUrl);await new Promise((r,j)=>{ws.onopen=r;ws.onerror=j;});let seq=0;const pending=new Map();ws.onmessage=e=>{const m=JSON.parse(e.data),p=pending.get(m.id);if(p){pending.delete(m.id);m.error?p.reject(Error(JSON.stringify(m.error))):p.resolve(m.result);}};const send=(method,params={})=>new Promise((resolve,reject)=>{const id=++seq;pending.set(id,{resolve,reject});ws.send(JSON.stringify({id,method,params}));});
  await send('Page.navigate',{url:'file:///'+path.join(dir,'rich-note-preview.html').replaceAll('\\','/')});
  for(let i=0;i<100;i++){if((await send('Runtime.evaluate',{expression:'!!document.getElementById("text")?.noteEditor',returnByValue:true})).result.value)break;await delay(100);}
  const samples=[];
  for(const count of [100,1000,4000]){
   const r=await send('Runtime.evaluate',{expression:`(async()=>{const e=document.getElementById('text').noteEditor;for(let i=0;i<${count};i++){const document=e.snapshot();document.doc.content[0].content[0].text='持续替换中文 '+i;e.load({document,text:''});if(i%50===0)await new Promise(requestAnimationFrame);}await new Promise(requestAnimationFrame);return e.value;})()`,awaitPromise:true,returnByValue:true});if(r.exceptionDetails)throw Error(JSON.stringify(r.exceptionDetails));
   const before={heap:await send('Runtime.getHeapUsage'),dom:await send('Memory.getDOMCounters')};
   await send('HeapProfiler.collectGarbage');await delay(100);
   samples.push({additionalReplacements:count,before,after:{heap:await send('Runtime.getHeapUsage'),dom:await send('Memory.getDOMCounters')}});
  }
  const name=process.argv[2]||'m7-editor-retention-probe';if(!/^[a-z0-9-]+$/.test(name))throw Error('Evidence name only');const output=path.join(dir,name+'.json');if(fs.existsSync(output))throw Error('Preserve prior probe output');fs.writeFileSync(output,JSON.stringify({scope:'Disposable headless browser with actual editor assets, mocked host, explicit GC after batches; not capacity performance or native physical validation',samples},null,2));
  console.log(JSON.stringify(samples.map(s=>({replacements:s.additionalReplacements,retainedMiB:s.after.heap.usedSize/1048576,nodes:s.after.dom.nodes,listeners:s.after.dom.jsEventListeners}))));await send('Browser.close');
 }finally{ws?.close();await delay(300);if(child.exitCode===null)child.kill();}
})().catch(e=>{console.error(e);process.exitCode=1});
