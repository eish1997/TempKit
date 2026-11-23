const {test}=require('node:test'),assert=require('node:assert/strict');
const create=require('../ui/display-watch.js');
test('unchanged monitor metadata does not reposition; scale/workarea/disconnect each refresh',async()=>{
 let value=['monitor-1:scale1:1920x1040'],calls=0;
 const check=create(async()=>value,async()=>calls++);
 await check();await check();assert.equal(calls,0);
 for(const next of [['monitor-1:scale1.5:1280x690'],['monitor-1:scale1.5:1280x650'],['monitor-2:scale1:1920x1040']]){
  value=next;await check();await check();
 }assert.equal(calls,3);
});
test('slow metadata read cannot overlap; failed refresh retries next check',async()=>{
 let resolve,reads=0,calls=0,fail=true;
 const check=create(()=>{reads++;return new Promise(r=>resolve=r)},async()=>{calls++;if(fail)throw Error('changing monitor')});
 const first=check();await check();assert.equal(reads,1);resolve(['old']);await first;
 let next=check();resolve(['new']);await assert.rejects(next,/changing monitor/);
 fail=false;next=check();resolve(['new']);await next;assert.equal(calls,2);
 next=check();resolve(['new']);await next;assert.equal(calls,2);
});
