'use strict';
(async()=>{
 const invoke=window.__TAURI__.core.invoke,id=new URLSearchParams(location.search).get('id');
 const trace=step=>{if(new URLSearchParams(location.search).get('generationTrace')==='1')invoke('board_capacity_progress',{result:{source:'note',id,step}}).catch(()=>{});};
 trace('script-start');
 const listen=async(name,handler)=>{trace('listen-before:'+name);const unlisten=await window.__TAURI__.event.listen(name,handler,{target:{kind:'WebviewWindow',label:id}});trace('listen-after:'+name);return unlisten;};
 const input=document.getElementById('text'),status=document.getElementById('status'),retry=document.getElementById('retry'),top=document.getElementById('top'),remove=document.getElementById('delete');
 if(window.quickgenIcons?.pin)top.innerHTML=window.quickgenIcons.pin;
 const paintTop=()=>top.setAttribute('aria-pressed',String(!!note?.always_on_top));
 let finishInitialLoad;const initialLoad=new Promise(resolve=>finishInitialLoad=resolve);
 let lastError;
 const report=text=>{status.textContent=text;if(text!==lastError){lastError=text;invoke('note_report_error',{error:text||null}).catch(()=>{});}};
 let note,composing=false,timer,tail=Promise.resolve(),saved='',actionTail=Promise.resolve(),actionBusy=false,barrier=false;
 const flush=()=>{clearTimeout(timer);const work=tail.then(async()=>{
  await initialLoad;
  if(!note)throw Error('便签尚未载入，请先重试载入');
  if(note.deleted)return;
  if(composing)throw Error('输入法组合尚未结束，请完成输入后重试');
  const text=input.value;if(text===saved){await invoke('note_save_geometry');report('');return;}
  note=await invoke('note_patch',{id,patch:{field:'text',revision:note.text_revision,text}});saved=text;await invoke('note_save_geometry');report('');
 });tail=work.catch(error=>{report('保存失败，正文仍保留：'+error);});return work;};
 const action=(work,label)=>{
  if(actionBusy||barrier||note?.deleted)return Promise.resolve();
  actionBusy=true;input.readOnly=true;
  actionTail=(async()=>{try{await flush();await work();}catch(error){report(label+'：'+error);}finally{actionBusy=false;input.readOnly=barrier||!!note?.deleted;top.disabled=!note||!!note.deleted;remove.disabled=!note||!!note.deleted;}})();
  return actionTail;
 };
 const hide=()=>action(()=>invoke('note_hide'),'无法收起');
 input.addEventListener('compositionstart',()=>composing=true);
 input.addEventListener('compositionend',()=>{composing=false;timer=setTimeout(()=>flush().catch(()=>{}),250)});
 input.addEventListener('input',()=>{if(composing)return;clearTimeout(timer);timer=setTimeout(()=>flush().catch(()=>{}),250)});
 document.getElementById('hide').onclick=hide;
 remove.onclick=()=>action(async()=>{note=await invoke('note_delete');clearTimeout(timer);},'无法删除');
 top.onclick=()=>action(async()=>{note=await invoke('note_set_top',{enabled:!note.always_on_top});paintTop();},'无法更改置顶');
 const menu=document.getElementById('note-menu'),more=document.getElementById('more');
 more.onclick=()=>{menu.hidden=!menu.hidden;more.setAttribute('aria-expanded',String(!menu.hidden));};
 document.getElementById('note-header').onpointerdown=e=>{if(e.button!==0||e.target.closest('button')||barrier||actionBusy)return;e.preventDefault();invoke('note_drag').catch(error=>report('无法移动：'+error));};
 document.getElementById('resize').onpointerdown=e=>{if(e.button!==0||barrier||actionBusy)return;e.preventDefault();invoke('note_resize').catch(error=>report('无法调整尺寸：'+error));};
 const load=async()=>{
  retry.disabled=true;input.disabled=true;
  try{
   trace('load-before-get');const loaded=await invoke('note_get',{id});trace('load-after-get');
   note=loaded;input.value=saved=loaded.text;input.disabled=false;top.disabled=false;remove.disabled=false;paintTop();retry.hidden=true;report('');input.focus();
  }catch(error){trace('load-error:'+String(error));report('无法载入便签：'+error);retry.hidden=false;}
  finally{retry.disabled=false;}
 };
 retry.onclick=load;
 await listen('note-hide-request',hide);
 await listen('note-flush-request',async event=>{trace('flush-request');let error=null;barrier=true;input.readOnly=true;try{await actionTail;await flush()}catch(e){error=String(e)}finally{if(error){barrier=false;input.readOnly=actionBusy||!!note?.deleted}}await invoke('note_flush_ack',{token:event.payload,error});});
 await listen('note-flush-release',()=>{barrier=false;input.readOnly=actionBusy||!!note?.deleted});
 await listen('note-geometry-changed',()=>{if(!note||actionBusy||barrier||note.deleted)return;clearTimeout(timer);timer=setTimeout(()=>flush().catch(()=>{}),250)});
 trace('before-diagnostic');const testMode=await invoke('board_diagnostic');trace('after-diagnostic');
 if(['notes-capacity','notes-fault'].includes(testMode))await listen('note-capacity-edit',async event=>{
  let error=null;try{if(barrier||actionBusy||!note||note.deleted)throw Error('capacity editor unavailable');input.value=event.payload.text;await flush();}catch(e){error=String(e)}
  await invoke('note_capacity_ack',{token:event.payload.token,error,draft:input.value,status:status.textContent});
 });
 if(['notes-fault','notes-restart'].includes(testMode))await listen('note-test-reload',async event=>{await load();await invoke('note_ready');await invoke('note_capacity_ack',{token:event.payload,error:note?null:status.textContent,draft:input.value,status:status.textContent});});
 await load();finishInitialLoad();trace('initial-load-finished');
 await invoke('note_ready');
 if(note)input.focus();
})();
