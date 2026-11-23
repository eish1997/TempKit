const assert=require('node:assert/strict'),vm=require('node:vm'),fs=require('node:fs'),path=require('node:path');
(async()=>{
 const events={},els={text:{value:'',focus(){},addEventListener(){}},status:{},hide:{},retry:{},top:{setAttribute(){}},delete:{}};
 let release,started;const began=new Promise(r=>started=r),gate=new Promise(r=>release=r);let ack=0,deleted=false,hidden=0;
 const invoke=async(name)=>{if(name==='note_get')return {text:'saved',text_revision:0};if(name==='note_delete'){started();await gate;deleted=true;return {text:'saved',deleted:true};}if(name==='note_hide')hidden++;if(name==='note_flush_ack')ack++;};
 await vm.runInNewContext(fs.readFileSync(path.join(__dirname,'../ui/note.js'),'utf8'),{URLSearchParams,location:{search:'?id=note-test'},setTimeout,clearTimeout,document:{getElementById:id=>els[id]||{setAttribute(){}}},window:{__TAURI__:{core:{invoke},event:{listen:async(k,v)=>events[k]=v}}}});
 const deletion=els.delete.onclick();await began;const barrier=events['note-flush-request']({payload:1});await Promise.resolve();assert.equal(ack,0,'global flush must wait for pending delete');await els.hide.onclick();assert.equal(hidden,0);release();await deletion;await barrier;assert.equal(deleted,true);assert.equal(ack,1);events['note-flush-release']();assert.equal(els.text.readOnly,true);
 console.log('Global save waits for pending delete; competing hide is refused; deleted editor remains locked.');
})().catch(e=>{console.error(e);process.exitCode=1});
