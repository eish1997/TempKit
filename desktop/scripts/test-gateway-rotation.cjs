const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict');
(async()=>{
 const source=fs.readFileSync('desktop/ui/gateway-rotation.js','utf8');
 async function fixture(invoke){
  const button={},status={},back={},target={},form={elements:{account:{value:'same-account'},token:{value:'synthetic-new'}},querySelector:()=>button};
  const content={set innerHTML(value){assert(value.includes('type="password"'));assert(value.includes('账号一致性由你确认'));},querySelector:s=>({'form':form,'#rotation-status':status,'#rotation-back':back,'#rotation-target':target})[s]};
  const errors=[],context={window:{quickgenGatewaySettings:{async mount(){}}}};vm.runInNewContext(source,context);
  await context.window.quickgenGatewayRotation.mount(content,invoke,e=>errors.push(String(e)),{id:'c',credential_ref:'version-old'});
  return {form,button,status,back,errors,submit:()=>form.onsubmit({preventDefault(){}})};
 }
 let calls=[],resolve;
 const ui=await fixture((name,args)=>{calls.push({name,args});assert.equal(ui.form.elements.token.value,'');return new Promise(r=>resolve=r);});
 assert.equal(calls.length,0);const pending=ui.submit();assert(ui.button.disabled);await ui.submit();assert.equal(calls.length,1);
 assert.deepEqual(JSON.parse(JSON.stringify(calls[0])),{name:'gateway_rotate',args:{id:'c',expectedRef:'version-old',accountId:'same-account',token:'synthetic-new'}});
 resolve({});await pending;assert(ui.button.disabled);assert(ui.status.textContent.includes('重新检测'));assert.equal(ui.form.elements.token.value,'');
 const failure=await fixture(async()=>{throw Error('credential version conflict')});await failure.submit();assert.equal(failure.errors.length,1);assert.equal(failure.form.elements.token.value,'');assert.equal(failure.button.disabled,false);
 let lateReject;const stale=await fixture(()=>new Promise((_,reject)=>lateReject=reject));const stalePending=stale.submit();stale.back.onclick();lateReject(Error('late failure'));await stalePending;assert.equal(stale.errors.length,0);assert.equal(stale.status.textContent,undefined);
 assert(calls.every(c=>c.name==='gateway_rotate'));
 console.log('Rotation UI: explicit same-account confirmation, version-bound one-shot secret, duplicate suppression, failure clearing, stale response isolation; no model call passed');
})().catch(error=>{console.error(error);process.exitCode=1});
