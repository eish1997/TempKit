const assert=require('node:assert/strict'),{harness,record}=require('./note-test-harness.cjs');
(async()=>{let release,entered,ack,patches=0;const reading=new Promise(r=>entered=r),gate=new Promise(r=>release=r);
 const h=harness(async(name,args)=>{if(name==='note_get'){entered();await gate;return record('已保存正文',{text_revision:4});}if(name==='note_patch')patches++;if(name==='note_flush_ack')ack=args;});
 try{const init=h.start();await reading;const flushing=h.events['note-flush-request']({payload:1});await new Promise(r=>setImmediate(r));assert.equal(ack,undefined);release();await init;await flushing;assert.equal(ack.error,null);assert.equal(h.elements.text.value,'已保存正文');assert.equal(patches,0);assert.equal(h.elements.text.readOnly,true);console.log('Initial structured load waits and remains locked during global barrier.');}finally{h.close();}
})().catch(e=>{console.error(e);process.exitCode=1});
