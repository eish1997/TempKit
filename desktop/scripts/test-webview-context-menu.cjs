const {test}=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
test('browser default is cancelled while custom menu handlers and other input events remain active',()=>{
 const target=new EventTarget();vm.runInNewContext(fs.readFileSync('desktop/ui/webview-context-menu.js','utf8'),{window:target});
 let custom=0;target.addEventListener('contextmenu',()=>custom++);
 const menu=new Event('contextmenu',{cancelable:true});target.dispatchEvent(menu);assert.equal(menu.defaultPrevented,true);assert.equal(custom,1);
 for(const type of ['keydown','pointerdown','copy','paste','drop']){const event=new Event(type,{cancelable:true});target.dispatchEvent(event);assert.equal(event.defaultPrevented,false);}
});
test('all product pages load the shared policy before application code',()=>{
 for(const name of ['board','manager','note','ink','index']){const html=fs.readFileSync('desktop/ui/'+name+'.html','utf8');assert.equal((html.match(/src="webview-context-menu.js"/g)||[]).length,1,name);assert(html.indexOf('src="webview-context-menu.js"')<html.indexOf('src="'+({'index':'app'}[name]||name)+'.js"'));}
});
