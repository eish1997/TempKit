const assert=require('node:assert/strict'),vm=require('node:vm'),fs=require('node:fs');
(async()=>{
 const button={},form={elements:{amount:{value:''}},querySelector:()=>button},status={},back={},calls=[],errors=[];
 const content={innerHTML:'',querySelector:s=>s==='form'?form:s==='#budget-status'?status:back};
 let saved=null,readFailure=false;const invoke=async(name,args)=>{calls.push({name,args});assert.equal(name,'image_budget');if(readFailure)throw Error('isolated failure');if(args.limitMicros!==undefined){saved=args.limitMicros;}return {limit_micros:saved,reserved_micros:0};};
 const window={quickgenGatewaySettings:{mount:async()=>{}}};vm.runInNewContext(fs.readFileSync('desktop/ui/image-budget-settings.js','utf8'),{window});
 await window.quickgenImageBudget.mount(content,invoke,e=>errors.push(String(e)));
 assert.equal(calls.length,1);assert.equal(saved,null);assert.equal(button.disabled,false);assert.match(status.textContent,/尚未设置/);
 for(const bad of ['0','-1','1e2','0.0000001','NaN','1000000.000001']){form.elements.amount.value=bad;await form.onsubmit({preventDefault(){}});assert.equal(saved,null);}
 assert.equal(calls.length,1);assert.equal(errors.length,6);
 form.elements.amount.value='2.000001';await form.onsubmit({preventDefault(){}});assert.equal(saved,2000001);assert.equal(button.disabled,true);assert.equal(form.elements.amount.disabled,true);assert.match(status.textContent,/2\.000001/);
 await form.onsubmit({preventDefault(){}});assert.equal(calls.length,2,'saved cap cannot be reset');
 readFailure=true;await window.quickgenImageBudget.mount(content,invoke,e=>errors.push(String(e)));assert.equal(button.disabled,true,'failed read must not enable overwrite');
 let finish;const pending=new Promise(r=>finish=r);const mount=window.quickgenImageBudget.mount(content,()=>pending,e=>{throw e});back.onclick();status.textContent='new page';finish({limit_micros:2,reserved_micros:1});await mount;assert.equal(status.textContent,'new page');
 console.log('Budget settings: read-only mount, exact micros, invalid input, immutable cap and stale response isolation passed');
})().catch(error=>{console.error(error);process.exitCode=1});
