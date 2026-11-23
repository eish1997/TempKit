const {test}=require('node:test'),assert=require('node:assert/strict'),vm=require('node:vm'),fs=require('node:fs'),path=require('node:path');
const source=fs.readFileSync(path.join(__dirname,'../ui/theme.js'),'utf8');
async function fixture(buttonPresent=true){
 let config={theme:'system'},listener,failSave=false;const writes=[];
 const button={setAttribute(k,v){this[k]=v}},root={dataset:{}},error={};
 const invoke=async(cmd,args)=>{if(cmd==='load_config')return {...config};if(failSave)throw Error('保存失败');writes.push(args);config.theme=args.value;listener({payload:'theme'})};
 vm.runInNewContext(source,{document:{documentElement:root,getElementById:id=>id==='theme'?(buttonPresent?button:null):error},localStorage:{getItem:()=>null,setItem(){}},window:{quickgenIcons:{sun:'sun',moon:'moon',monitor:'monitor'},__TAURI__:{core:{invoke},event:{listen:async(_,fn)=>{listener=fn}}}}});
 const flush=async()=>{for(let i=0;i<10;i++)await Promise.resolve()};await flush();
 return {button,root,error,writes,flush,setFail:()=>failSave=true,external:async value=>{config.theme=value;listener({payload:'theme'});await flush()}};
}
test('theme cycles system/light/dark, persists and updates the accessible button',async()=>{const f=await fixture();for(const expected of ['light','dark','system']){await f.button.onclick();await f.flush();assert.equal(f.root.dataset.theme,expected);assert.equal(f.writes.at(-1).value,expected)}assert.match(f.button['aria-label'],/跟随系统/)});
test('failed persistence restores the last theme and permits retry',async()=>{const f=await fixture();f.setFail();await f.button.onclick();assert.equal(f.root.dataset.theme,'system');assert.equal(f.button.disabled,false);assert.match(f.error.textContent,/保存失败/)});
test('open utility webview follows the same host theme without a button',async()=>{const f=await fixture(false);await f.external('dark');assert.equal(f.root.dataset.theme,'dark');await f.external('light');assert.equal(f.root.dataset.theme,'light');await f.external('invalid');assert.equal(f.root.dataset.theme,'system')});
