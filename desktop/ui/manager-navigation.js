'use strict';
// A page owns its DOM and request lifetime; leaving never cancels a host mutation.
window.quickgenManagerNavigation={create({content,heading,back,message,invoke,onRootBack}){
 let current=null,stack=[];
 const expired=()=>Object.assign(new Error('页面已离开'),{stale:true});
 function dispose(){if(!current)return;current.alive=false;for(const input of current.root.querySelectorAll('input[type=password]'))input.value='';current.cleanup.forEach(fn=>fn());}
 async function render(page){
  dispose();message.textContent='';const root=document.createElement('section');content.replaceChildren(root);
  const scope={alive:true,root,cleanup:[]};current=scope;heading.textContent=page.title;back.hidden=!stack.length&&!onRootBack;
  const fail=error=>{if(scope.alive&&!error?.stale)message.textContent=String(error);};
  const call=async(command,args)=>{if(!scope.alive)throw expired();const result=await invoke(command,args);if(!scope.alive)throw expired();return result;};
  const context={root,invoke:call,fail,active:()=>scope.alive,onDispose:fn=>scope.cleanup.push(fn),push:next=>push(next),back:()=>pop()};
  scope.page=page;
  try{await call('manager_page_size',{compact:page.title==='预设库'});await page.mount(context);}catch(error){fail(error);}
 }
 function push(page){if(current)stack.push({page:current.page,focus:document.activeElement?.id});return render(page);}
 async function pop(){const previous=stack.pop();if(!previous){if(onRootBack)await onRootBack();return;}await render(previous.page);if(previous.focus)document.getElementById(previous.focus)?.focus();}
 back.onclick=pop;
 return {start:page=>{stack=[];return render(page);},push,back:pop,dispose};
}};
