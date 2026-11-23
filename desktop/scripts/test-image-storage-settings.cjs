const assert=require('node:assert/strict'),vm=require('node:vm'),fs=require('node:fs'),path=require('node:path');
(async()=>{
 const elements={},calls=[],errors=[];const content={innerHTML:'',querySelector:s=>elements[s]||(elements[s]={hidden:s==='#storage-open',disabled:false,textContent:'',scrollIntoView(){this.scrolled=true},focus(){this.focused=true}})};
 const status={staged_bytes:1048576,pending_bytes:1048576,limit_bytes:268435456,protected_files:1,residue_files:1};
 const drag={bytes:1048576,files:2,limit_bytes:536870912};
 let reject=false,back=0;const invoke=async(name,args)=>{calls.push({name,args});if(reject)throw Error('fixture unavailable');if(name==='image_storage'){if(args.action==='drag-status')return drag;if(args.action==='drag-archive')return {status:{...drag,files:1},archived:1,retained:1,directory:'isolated/drag-recovery'};if(args.action==='drag-clean')return {status:{...drag,files:1},removed:1,retained:1,deferred:1};return args.action==='status'?status:args.action==='clean'?{status,removed:2}:{status:{...status,residue_files:0},directory:'isolated/recovery',archived:1,retained:0};}};
 const window={confirm:()=>{throw Error('native confirmation must not be used')},quickgenGatewaySettings:{mount:async()=>{back++}}};vm.runInNewContext(fs.readFileSync(path.join(__dirname,'../ui/image-storage-settings.js'),'utf8'),{window});
 await window.quickgenImageStorage.mount(content,invoke,e=>errors.push(String(e)));
 assert.match(elements['#storage-usage'].textContent,/2.0 MiB/);assert.deepEqual(calls.map(c=>c.args.action),['status','drag-status'],'opening page must not mutate');
 assert.match(elements['#storage-drag-usage'].textContent,/512.0 MiB/);
 await elements['#storage-clean'].onclick();assert.match(elements['#storage-result'].textContent,/2/);
 await elements['#storage-archive'].onclick();assert.equal(elements['#storage-open'].hidden,false);assert.equal(elements['#storage-archive'].disabled,true);
 const before=calls.length;await elements['#storage-drag-clean'].onclick();assert.equal(calls.length,before,'opening confirmation must not delete');assert.equal(elements['#storage-confirm'].hidden,false);assert(elements['#storage-confirm'].scrolled);elements['#storage-confirm-cancel'].onclick();assert.equal(calls.length,before,'cancel must not delete');
 await elements['#storage-drag-clean'].onclick();await elements['#storage-confirm-accept'].onclick();assert.match(elements['#storage-result'].textContent,/已清理 1 个拖出副本/);assert.match(elements['#storage-result'].textContent,/待下一批/);assert.equal(elements['#storage-archive'].disabled,true,'drag completion must retain empty archive disabled state');
 const beforeBackup=calls.length;await elements['#storage-drag-archive'].onclick();assert.equal(calls.length,beforeBackup);assert.equal(elements['#storage-drag-clean'].disabled,true);await elements['#storage-confirm-accept'].onclick();assert.match(elements['#storage-result'].textContent,/已备份 1 个拖出文件/);assert.match(elements['#storage-result'].textContent,/仍占磁盘空间/);
 elements['#storage-open'].onclick();assert.equal(calls.at(-1).name,'board_recovery_open');assert.equal(calls.at(-1).args.directory,'isolated/drag-recovery');
 elements['#storage-drag-open'].onclick();assert.equal(calls.at(-1).name,'board_drag_exports_open');assert.equal(calls.at(-1).args,undefined);
 reject=true;await elements['#storage-refresh'].onclick();assert.equal(errors.length,2);assert.equal(elements['#storage-clean'].disabled,false);
 elements['#storage-back'].onclick();assert.equal(back,1);
 // A read resolving after navigation must not rewrite the new page.
 let finish;const pending=new Promise(r=>finish=r);const mounted=window.quickgenImageStorage.mount(content,()=>pending,e=>{throw e});
 elements['#storage-back'].onclick();elements['#storage-usage'].textContent='new page';finish(status);await mounted;assert.equal(elements['#storage-usage'].textContent,'new page');
 console.log('Storage settings: read-only mount, explicit maintenance, errors, verified-folder opening and stale read isolation passed.');
})().catch(e=>{console.error(e);process.exitCode=1});
