const assert=require('node:assert/strict'),vm=require('node:vm'),fs=require('node:fs'),path=require('node:path');
(async()=>{
 const events={},elements={text:{value:'',focus(){},addEventListener(){}},status:{},retry:{},top:{setAttribute(){}},delete:{}};
 let release,entered,ack,patches=0;const reading=new Promise(r=>entered=r),gate=new Promise(r=>release=r);
 const invoke=async(name,args)=>{if(name==='note_get'){entered();await gate;return {text:'已保存正文',text_revision:4}}if(name==='note_patch')patches++;if(name==='note_flush_ack')ack=args;};
 const init=vm.runInNewContext(fs.readFileSync(path.join(__dirname,'../ui/note.js'),'utf8'),{URLSearchParams,location:{search:'?id=note-test'},setTimeout,clearTimeout,document:{getElementById:id=>elements[id]||{setAttribute(){}}},window:{__TAURI__:{core:{invoke},event:{listen:async(k,v)=>events[k]=v}}}});
 await reading;const flushing=events['note-flush-request']({payload:1});
 await new Promise(r=>setImmediate(r));assert.equal(ack,undefined,'initial read still pending: flush must wait, not reject or claim saved');
 release();await init;await flushing;assert.equal(ack.error,null);assert.equal(elements.text.value,'已保存正文');assert.equal(patches,0,'must not replace saved text with unloaded blank draft');
 console.log('Flush during initial read waits, preserves stored body, and acknowledges only after load.');
})().catch(e=>{console.error(e);process.exitCode=1});
