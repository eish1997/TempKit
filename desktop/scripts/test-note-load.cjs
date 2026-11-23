const assert=require('node:assert/strict'),vm=require('node:vm'),fs=require('node:fs'),path=require('node:path');
(async()=>{
 const events={},elements={text:{value:'',focus(){},addEventListener(){}},status:{},hide:{},retry:{hidden:true},top:{setAttribute(){}},delete:{}};
 let unavailable=true,reads=0,patches=0,ack;
 const invoke=async(name,args)=>{
  if(name==='note_get'){reads++;if(unavailable)throw Error('isolated read failure');return {text:'已保存正文',text_revision:4};}
  if(name==='note_patch'){patches++;throw Error('unexpected write');}
  if(name==='note_flush_ack')ack=args;
 };
 await vm.runInNewContext(fs.readFileSync(path.join(__dirname,'../ui/note.js'),'utf8'),{URLSearchParams,location:{search:'?id=note-test'},setTimeout,clearTimeout,document:{getElementById:id=>elements[id]||{setAttribute(){}}},window:{__TAURI__:{core:{invoke},event:{listen:async(k,v,options)=>{assert.deepEqual(JSON.parse(JSON.stringify(options)),{target:{kind:'WebviewWindow',label:'note-test'}});events[k]=v}}}}});
 assert.equal(elements.text.disabled,true);
 assert.equal(elements.retry.hidden,false,'load failure must expose retry');
 await events['note-flush-request']({payload:1});assert(ack.error,'unloaded editor cannot acknowledge saved');
 unavailable=false;await elements.retry.onclick();
 assert.equal(elements.text.value,'已保存正文');assert.equal(elements.text.disabled,false);assert.equal(elements.retry.hidden,true);assert.equal(reads,2);assert.equal(patches,0);
 await events['note-flush-request']({payload:2});assert.equal(ack.error,null);
 console.log('Load failure exposes retry; failed flush refuses; retry restores persisted text without writing.');
})().catch(e=>{console.error(e);process.exitCode=1});
