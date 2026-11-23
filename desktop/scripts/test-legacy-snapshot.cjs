const assert=require('node:assert/strict');
const {persist}=require('../ui/legacy-snapshot.js');
(async()=>{
 let tx,stored;const db={transaction(name,mode){assert.equal(name,'scene');assert.equal(mode,'readwrite');return tx={objectStore(){return{put(value,key){assert.equal(key,'current');stored=value}}}}}};
 const source={items:[{id:'a',src:'data:x',x:12}],jobs:[],selection:['a']};
 let done=false;const pending=persist(db,source).then(value=>{done=true;return value});source.items[0].x=99;
 await Promise.resolve();assert.equal(done,false);assert.equal(stored.items[0].x,12);tx.oncomplete();assert.equal((await pending).items[0].x,12);
 const failed=persist(db,source);tx.onabort();await assert.rejects(failed,/aborted/);
 console.log('snapshot copy, transaction completion and abort passed');
})().catch(error=>{console.error(error);process.exitCode=1});
