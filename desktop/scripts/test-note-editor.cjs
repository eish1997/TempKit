const assert=require('node:assert/strict'),vm=require('node:vm'),fs=require('node:fs');
(async()=>{
 const events={},handlers={},input={value:'',readOnly:false,focus(){},addEventListener(k,v){handlers[k]=v}},status={},hide={},retry={},top={setAttribute(){}},remove={};
 let stored='',revision=0,fail=false,geometryFail=false,topFail=false,topEnabled=false,deleteFail=false,deleted=false;const calls=[];
 const invoke=async(name,args)=>{calls.push([name,args]);
  if(name==='note_save_geometry'&&geometryFail)throw Error('geometry disk full');
  if(name==='note_delete'){if(deleteFail)throw Error('delete failed');deleted=true;return {text:stored,text_revision:revision,deleted:true};}
  if(name==='note_get')return {text:stored,text_revision:revision,resident:true,always_on_top:topEnabled};
  if(name==='note_set_top'){if(topFail)throw Error('top failed');topEnabled=args.enabled;return {text:stored,text_revision:revision,resident:true,always_on_top:topEnabled};}
  if(name==='note_patch'){if(fail)throw Error('disk full');assert.equal(args.patch.revision,revision);stored=args.patch.text;return {text:stored,text_revision:++revision};}
 };
 const context=vm.createContext({URLSearchParams,location:{search:'?id=note-test'},setTimeout,clearTimeout,
  document:{getElementById:id=>({text:input,status,hide,retry,top,delete:remove}[id]||{setAttribute(){}})},window:{__TAURI__:{core:{invoke},event:{listen:async(k,v)=>events[k]=v}}}});
 await vm.runInContext(fs.readFileSync(require('node:path').join(__dirname,'../ui/note.js'),'utf8'),context);
 input.value='中文草稿';fail=true;await events['note-flush-request']({payload:1});
 assert.equal(stored,'');assert.equal(input.value,'中文草稿');assert.equal(input.readOnly,false);assert.match(status.textContent,/disk full/);
 fail=false;await events['note-flush-request']({payload:2});assert.equal(stored,'中文草稿');assert.equal(input.readOnly,true);
 events['note-flush-release']();assert.equal(input.readOnly,false);
 geometryFail=true;await events['note-flush-request']({payload:4});assert.match(status.textContent,/geometry disk full/);geometryFail=false;await events['note-flush-request']({payload:5});assert.equal(status.textContent,'');events['note-flush-release']();
 topFail=true;await top.onclick();assert.equal(topEnabled,false);assert.match(status.textContent,/top failed/);topFail=false;await top.onclick();assert.equal(topEnabled,true);await top.onclick();assert.equal(topEnabled,false);assert.equal(stored,'中文草稿');
 handlers.compositionstart();input.value='组合中';await events['note-flush-request']({payload:3});assert.equal(stored,'中文草稿');
 const ack=calls.filter(c=>c[0]==='note_flush_ack');assert(ack[0][1].error);assert.equal(ack[1][1].error,null);assert(ack[2][1].error);assert.equal(ack[3][1].error,null);assert(ack[4][1].error);
 // Composition blocks deletion; retry preserves the draft on persistence failure.
 await remove.onclick();assert.equal(deleted,false);assert.equal(input.readOnly,false);
 handlers.compositionend();deleteFail=true;await remove.onclick();assert.equal(deleted,false);assert.equal(stored,'组合中');assert.equal(input.readOnly,false);
 deleteFail=false;await remove.onclick();assert.equal(deleted,true);assert.equal(input.readOnly,true);
 const patchCount=calls.filter(c=>c[0]==='note_patch').length;await events['note-flush-request']({payload:8});events['note-flush-release']();assert.equal(input.readOnly,true);assert.equal(calls.filter(c=>c[0]==='note_patch').length,patchCount);
 console.log('Actual note editor: draft retained on failure, retry saved, flush freeze/release, composition refusal and independent top retry passed');
})().catch(e=>{console.error(e);process.exitCode=1});
