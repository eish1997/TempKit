const assert=require('node:assert/strict'),{harness,record,documentOf}=require('./note-test-harness.cjs');
(async()=>{
 let channel,ack,stored=record('原正文'),patches=0;
 const h=harness(async(name,args)=>{
  if(name==='board_diagnostic')return 'notes-rich-capacity';
  if(name==='note_capacity_subscribe'){channel=args.channel;return;}
  if(name==='note_get')return stored;
  if(name==='note_patch'){patches++;stored={...stored,document:args.patch.document,text_revision:stored.text_revision+1};return stored;}
  if(name==='note_capacity_ack')ack=args;
 });
 try{
  await h.start();assert(channel);assert(!h.events['note-capacity-edit'],'test payload must not subscribe every editor to a full-document event');
  const document=documentOf('中文定向编辑\n第二行');for(const p of document.doc.content)p.attrs={todo:false,done:false};document.doc.content[0].content[0].marks=[{type:'strong'}];
  await channel.onmessage(new TextEncoder().encode(JSON.stringify({document,text:'',token:42})).buffer);
  assert.equal(patches,1);assert.equal(ack.token,42);assert.equal(ack.error,null);assert.equal(ack.draft,'中文定向编辑\n第二行');assert.deepEqual(JSON.parse(JSON.stringify(stored.document)),document);
  await h.events['note-flush-request']({payload:43});assert.equal(patches,1,'save barrier must not repeat the committed edit');
 }finally{h.close();}
 let subscribed=false;const normal=harness(async(name)=>{if(name==='board_diagnostic')return '';if(name==='note_capacity_subscribe')subscribed=true;if(name==='note_get')return record();});
 try{await normal.start();assert.equal(subscribed,false);}finally{normal.close();}
 console.log('Diagnostic channel UTF-8 rich save / acknowledgement / barrier / default-mode isolation passed.');
})().catch(e=>{console.error(e);process.exitCode=1});
