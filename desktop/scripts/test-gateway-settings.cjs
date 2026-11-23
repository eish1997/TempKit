const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict');
(async()=>{
 let buttons=[],form={elements:{address:{value:'http://127.0.0.1:3000'},account:{value:'account-a'},token:{value:'synthetic-only'}},querySelector:()=>button};
 const button={disabled:false},list={replaceChildren(){},append(){}};
 const content={set innerHTML(value){assert(value.includes('type="password"'));},querySelector:selector=>selector==='form'?form:list};
 const context={window:{},crypto:{randomUUID:()=> 'fixture-id'},document:{createElement:()=>({append(){},set textContent(value){}})}};
 vm.runInNewContext(fs.readFileSync('desktop/ui/gateway-settings.js','utf8'),context);
 let requests=[],errors=[];
 await context.window.quickgenGatewaySettings.mount(content,async(name,args)=>{requests.push({name,args});if(name==='gateway_configure'){assert.equal(form.elements.token.value,'');throw Error('controlled failure');}return [];},e=>errors.push(String(e)));
 await form.onsubmit({preventDefault(){}});
 assert.equal(requests[1].args.profile.account_id,'account-a');assert.equal(requests[1].args.token,'synthetic-only');assert.equal(form.elements.token.value,'');assert.equal(button.disabled,false);assert.equal(errors.length,1);
 assert(requests.every(r=>!r.name.includes('task')));
 console.log('Connection form: masked one-shot credential, account binding, failure clearing/unlock, no task submission passed');
})().catch(e=>{console.error(e);process.exitCode=1});
