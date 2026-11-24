const assert=require('node:assert/strict'),{harness,record}=require('./note-test-harness.cjs');
(async()=>{
 let stored=record(),fail=false,geometryFail=false,topFail=false,deleteFail=false;const calls=[];
 const invoke=async(name,args)=>{calls.push([name,args]);
  if(name==='note_save_geometry'&&geometryFail)throw Error('geometry disk full');
  if(name==='note_delete'){if(deleteFail)throw Error('delete failed');return stored={...stored,deleted:true};}
  if(name==='note_get')return structuredClone(stored);
  if(name==='note_set_top'){if(topFail)throw Error('top failed');return stored={...stored,always_on_top:args.enabled};}
  if(name==='note_patch'){if(fail)throw Error('disk full');assert.equal(args.patch.field,'document');assert.equal(args.patch.revision,stored.text_revision);const document=JSON.parse(JSON.stringify(args.patch.document));return stored={...stored,document,text:document.doc.content.map(p=>p.content?.map(n=>n.text||n.attrs?.label||'').join('')||'').join('\n'),text_revision:stored.text_revision+1};}
 };
 const h=harness(invoke);try{await h.start();const {text:input,status,top,delete:remove}=h.elements,events=h.events;
 input.value='中文草稿';fail=true;await events['note-flush-request']({payload:1});assert.equal(stored.text,'');assert.equal(input.value,'中文草稿');assert.equal(input.readOnly,false);assert.match(status.textContent,/disk full/);
 fail=false;await events['note-flush-request']({payload:2});assert.equal(stored.text,'中文草稿');assert.equal(input.readOnly,true);events['note-flush-release']();assert.equal(input.readOnly,false);
 geometryFail=true;await events['note-flush-request']({payload:4});assert.match(status.textContent,/geometry disk full/);geometryFail=false;await events['note-flush-request']({payload:5});assert.equal(status.textContent,'');events['note-flush-release']();
 topFail=true;await top.onclick();assert.equal(stored.always_on_top,false);assert.match(status.textContent,/top failed/);topFail=false;await top.onclick();assert.equal(stored.always_on_top,true);await top.onclick();assert.equal(stored.always_on_top,false);
 h.composition('compositionstart');input.value='组合中';await events['note-flush-request']({payload:3});assert.equal(stored.text,'中文草稿');const ack=calls.filter(c=>c[0]==='note_flush_ack');assert(ack[0][1].error);assert.equal(ack[1][1].error,null);assert(ack[2][1].error);assert.equal(ack[3][1].error,null);assert(ack[4][1].error);
 await remove.onclick();assert(!stored.deleted);assert.equal(input.readOnly,false);h.composition('compositionend');deleteFail=true;await remove.onclick();assert(!stored.deleted);assert.equal(stored.text,'组合中');assert.equal(input.readOnly,false);
 deleteFail=false;await remove.onclick();assert(stored.deleted);assert.equal(input.readOnly,true);const count=calls.filter(c=>c[0]==='note_patch').length;await events['note-flush-request']({payload:8});events['note-flush-release']();assert.equal(input.readOnly,true);assert.equal(calls.filter(c=>c[0]==='note_patch').length,count);
 console.log('Real structured editor: persistence failure/retry, geometry failure, independent top, composition gate, delete barrier passed.');
 }finally{h.close();}
})().catch(e=>{console.error(e);process.exitCode=1});
