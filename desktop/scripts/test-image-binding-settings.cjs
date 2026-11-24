const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict');
(async()=>{
 const select=()=>({value:'',options:[],append(option){this.options.push(option);if(!this.value)this.value=option.value;}});
 const elements={preset:select(),connection:select(),model:{value:'fixture-model'},size:{value:'1024x1024'},quality:{value:''},format:{value:'b64_json'}};
 const button={disabled:false},status={},back={},remove={},form={elements,querySelector:()=>button};
 const extra={};const content={set innerHTML(value){assert(value.includes('费用核验'));},querySelector:key=>key==='form'?form:key==='#binding-status'?status:key==='#binding-remove'?remove:key==='#binding-back'?back:(extra[key]||={})};
 const context={window:{},document:{createElement:()=>({})}},calls=[],errors=[];
 vm.runInNewContext(fs.readFileSync('desktop/ui/image-binding-settings.js','utf8'),context);
 const invoke=async(name,args)=>{calls.push({name,args});if(name==='load_config')return {abilities:[{id:'clay',name:'保留原指令'}]};if(name==='gateway_connections')return [{id:'revoked',status:'revoked'},{id:'ready',status:'ready'}];if(name==='gateway_bind_preset')return {capability:{reference:{connection_id:'ready',capability_id:'image-edit',revision:'v1'}}};if(name==='gateway_unbind_preset')return null;throw Error(name);};
 await context.window.quickgenImageBindingSettings.mount(content,invoke,e=>errors.push(String(e)));
 assert.equal(elements.connection.options.length,2);elements.model.value='fixture-model';elements.size.value='1024x1024';elements.format.value='b64_json';await form.onsubmit({preventDefault(){}});
 const receipt=calls.find(c=>c.name==='gateway_bind_preset');assert.deepEqual(JSON.parse(JSON.stringify(receipt.args)),{connectionId:'ready',presetId:'clay',expected:{imageBinding:null,imageParameters:{}},policy:{model:'fixture-model',sizes:['1024x1024'],qualities:[],response_format:'b64_json'},parameters:{size:'1024x1024'}});
 assert(status.textContent.includes('未调用模型'));assert.equal(errors.length,0);assert(!calls.some(call=>call.name.includes('task')));assert.equal(button.disabled,false);
 await remove.onclick();assert(calls.some(call=>call.name==='gateway_unbind_preset'));assert(status.textContent.includes('已有任务与结果保留'));assert.equal(remove.disabled,true);
 elements.preset.options=[];elements.preset.value='';elements.connection.options=[];elements.connection.value='';
 const ref=id=>({connection_id:id==='a'?'revoked':'ready',capability_id:'image-edit',revision:id});
 const binding=id=>({capability:{reference:ref(id)},policy:{model:'model-'+id,response_format:'url'}});
 let releaseLate;
 await context.window.quickgenImageBindingSettings.mount(content,async(name,args)=>{
  if(name==='load_config')return {abilities:['a','b'].map(id=>({id,name:id,imageBinding:ref(id),imageParameters:{size:'saved-'+id}}))};
  if(name==='gateway_connections')return [{id:'revoked',status:'revoked'},{id:'ready',status:'ready'}];
  if(name==='gateway_image_binding')return args.reference.revision==='a'?binding('a'):new Promise(resolve=>releaseLate=resolve);
  throw Error(name);
 },e=>errors.push(String(e)));
 assert.equal(elements.model.value,'model-a');assert.equal(elements.size.value,'saved-a');assert.equal(button.disabled,true);assert.equal(remove.disabled,false);assert(status.textContent.includes('已撤销'));
 elements.preset.value='b';const pending=elements.preset.onchange();elements.preset.value='a';await elements.preset.onchange();releaseLate(binding('b'));await pending;
 assert.equal(elements.model.value,'model-a');assert.equal(elements.size.value,'saved-a');assert.equal(button.disabled,true);assert.equal(errors.length,0);
 console.log('Image binding: exact options, unbind, historical/revoked readback and stale selection isolation passed');
})().catch(e=>{console.error(e);process.exitCode=1});
