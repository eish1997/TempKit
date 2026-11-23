// Form-state regression only; actual WebView decoding and native panels require UI validation.
const {test}=require('node:test');
const assert=require('node:assert/strict');
const vm=require('node:vm');
const fs=require('node:fs');
const source=fs.readFileSync(require('node:path').join(__dirname,'../ui/manager.js'),'utf8');
function fixture(invokeOverride){
 const saves=[],decodes=[];
 const submit={disabled:false};
 const form={elements:{name:{value:'测试能力'},prompt:{value:'测试处理指令'},cover:{disabled:false}},querySelector:()=>submit};
 const elements={content:{querySelector:()=>form},error:{},dismiss:{},heading:{},cancel:{}};
 const context={URLSearchParams,DOMException,location:{search:'?mode=add'},crypto:{randomUUID:()=> 'test-ability'},quickgenIcons:{x:''},
  document:{getElementById:id=>elements[id]},
  window:{__TAURI__:{core:{invoke:async(command,args)=>{if(invokeOverride)await invokeOverride(command,args);if(command==='load_config')return {abilities:[]};if(command==='add_ability')saves.push(args);}}}},
  FileReader:class{readAsDataURL(file){this.result='data:'+file.type+';base64,'+file.name;this.onload()}},
  Image:class{decode(){return new Promise((resolve,reject)=>decodes.push({resolve,reject,src:this.src}))}}
 };
 vm.runInNewContext(source,context);
 const select=file=>form.elements.cover.onchange({target:{files:file?[file]:[]}});
 const flush=async()=>{for(let i=0;i<5;i++)await Promise.resolve()};
 return {saves,decodes,select,flush,submit:()=>form.onsubmit({preventDefault(){}}),form,elements};
}
const png=name=>({name,type:'image/png',size:100});
test('invalid selected cover blocks save; explicitly clearing it permits no-cover creation',async()=>{
 const f=fixture();f.select({name:'bad.txt',type:'text/plain',size:1});await f.submit();assert.equal(f.saves.length,0);assert.match(f.elements.error.textContent,/PNG/);
 f.select(null);await f.submit();assert.equal(f.saves.length,1);assert.equal(f.saves[0].ability.cover,'');
});
test('submission waits for decode and rejects a corrupt image',async()=>{
 const f=fixture();f.select(png('broken'));await f.flush();const pending=f.submit();assert.equal(f.form.elements.cover.disabled,true);assert.equal(f.saves.length,0);
 f.decodes[0].reject(new DOMException('bad image'));await pending;assert.equal(f.saves.length,0);assert.equal(f.form.elements.cover.disabled,false);
});
test('older decode cannot replace a newer cover selection',async()=>{
 const f=fixture();f.select(png('older'));await f.flush();f.select(png('newer'));await f.flush();
 f.decodes[1].resolve();await f.flush();f.decodes[0].resolve();await f.flush();await f.submit();assert.equal(f.saves[0].ability.cover,'data:image/png;base64,newer');
});
test('a fresh valid cover recovers from an earlier failure',async()=>{
 const f=fixture();f.select({type:'text/plain',size:1});await f.submit();f.select(png('valid'));await f.flush();f.decodes[0].resolve();await f.submit();assert.equal(f.saves.length,1);
});

test('a failed close after successful save retries only closing, without adding another card',async()=>{
 let closes=0;
 const f=fixture(async command=>{if(command==='close_manager' && ++closes===1)throw Error('关闭失败')});
 await f.submit();assert.equal(f.saves.length,1);assert.match(f.elements.error.textContent,/关闭失败/);
 assert.equal(f.form.elements.name.disabled,true);assert.equal(f.form.elements.prompt.disabled,true);
 await f.submit();assert.equal(closes,2);assert.equal(f.saves.length,1);
});
test('a failed save retains editable input and can be retried',async()=>{
 let attempts=0;
 const f=fixture(async command=>{if(command==='add_ability' && ++attempts===1)throw Error('写入失败')});
 await f.submit();assert.equal(f.saves.length,0);assert.equal(f.form.elements.cover.disabled,false);
 assert.equal(f.form.elements.name.value,'测试能力');assert.notEqual(f.form.elements.name.disabled,true);
 await f.submit();assert.equal(f.saves.length,1);
});
