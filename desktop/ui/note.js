'use strict';
(async()=>{
 const invoke=window.__TAURI__.core.invoke,id=new URLSearchParams(location.search).get('id');
 const trace=step=>{if(new URLSearchParams(location.search).get('generationTrace')==='1')invoke('board_capacity_progress',{result:{source:'note',id,step}}).catch(()=>{});};
 trace('script-start');
 const listen=async(name,handler)=>{trace('listen-before:'+name);const unlisten=await window.__TAURI__.event.listen(name,handler,{target:{kind:'WebviewWindow',label:id}});trace('listen-after:'+name);return unlisten;};
 const input=window.createQuickgenNoteEditor(document.getElementById('text')),status=document.getElementById('status'),retry=document.getElementById('retry'),top=document.getElementById('top'),remove=document.getElementById('delete');
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
  input.prepareSave();
  const document=input.snapshot(),key=JSON.stringify(document);if(key===saved){await invoke('note_save_geometry');report(input.error);return;}
  if(input.error)throw Error(input.error);
  note=await invoke('note_patch',{id,patch:{field:'document',revision:note.text_revision,document}});saved=input.key(note.document);input.acknowledge(note.document);await invoke('note_save_geometry');report('');
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
 input.addEventListener('note-interaction-error',event=>{const notice=document.getElementById('interaction-error');notice.textContent=event.detail;notice.hidden=false;});
 document.getElementById('hide').onclick=hide;
 remove.onclick=()=>action(async()=>{note=await invoke('note_delete');clearTimeout(timer);},'无法删除');
 top.onclick=()=>action(async()=>{note=await invoke('note_set_top',{enabled:!note.always_on_top});paintTop();},'无法更改置顶');
 const menu=document.getElementById('note-menu'),more=document.getElementById('more');
 const themeButton=document.getElementById('note-theme'),themeMenu=document.getElementById('note-theme-menu');
 const paintTheme=()=>{window.quickgenNoteAppearance.apply(document.body,note?.theme);for(const button of themeMenu.children)button.setAttribute('aria-pressed',String(button.dataset.color===note?.theme));};
 for(const color of window.quickgenNoteAppearance.themes){const button=document.createElement('button');button.type='button';button.dataset.color=color;button.setAttribute('aria-label',({default:'默认',yellow:'黄色',green:'绿色',blue:'蓝色',pink:'粉色',purple:'紫色',paper:'纸张'})[color]);button.onpointerdown=e=>e.preventDefault();button.onclick=()=>action(async()=>{note=await invoke('note_patch',{id,patch:{field:'theme',revision:note.theme_revision,theme:color}});paintTheme();themeMenu.hidden=true;themeButton.setAttribute('aria-expanded','false');},'无法更改主题');themeMenu.append(button);}
 themeButton.onclick=()=>{if(!note||actionBusy||barrier)return;themeMenu.hidden=!themeMenu.hidden;themeButton.setAttribute('aria-expanded',String(!themeMenu.hidden));};
 more.onclick=()=>{menu.hidden=!menu.hidden;more.setAttribute('aria-expanded',String(!menu.hidden));};
 document.getElementById('note-header').onpointerdown=e=>{if(e.button!==0||e.target.closest('button')||barrier||actionBusy)return;e.preventDefault();invoke('note_drag').catch(error=>report('无法移动：'+error));};
 document.getElementById('resize').onpointerdown=e=>{if(e.button!==0||barrier||actionBusy)return;e.preventDefault();invoke('note_resize').catch(error=>report('无法调整尺寸：'+error));};
 const load=async()=>{
  retry.disabled=true;input.disabled=true;
  try{
   trace('load-before-get');const loaded=await invoke('note_get',{id});trace('load-after-get');
   note=loaded;const formatError=input.load(loaded);saved=JSON.stringify(input.snapshot());input.disabled=false;top.disabled=false;remove.disabled=false;paintTop();paintTheme();retry.hidden=true;report(formatError);input.focus();
  }catch(error){trace('load-error:'+String(error));report('无法载入便签：'+error);retry.hidden=false;}
  finally{retry.disabled=false;}
 };
 retry.onclick=load;
 await listen('note-hide-request',hide);
 await listen('note-flush-request',async event=>{trace('flush-request');let error=null;barrier=true;input.readOnly=true;try{await actionTail;await flush()}catch(e){error=String(e)}finally{if(error){barrier=false;input.readOnly=actionBusy||!!note?.deleted}}await invoke('note_flush_ack',{token:event.payload,error});});
 await listen('note-flush-release',()=>{barrier=false;input.readOnly=actionBusy||!!note?.deleted});
 await listen('note-geometry-changed',()=>{if(!note||actionBusy||barrier||note.deleted)return;clearTimeout(timer);timer=setTimeout(()=>flush().catch(()=>{}),250)});
 trace('before-diagnostic');const testMode=await invoke('board_diagnostic');trace('after-diagnostic');
 if(['notes-capacity','notes-fault','notes-rich-smoke','notes-rich-capacity'].includes(testMode)){const channel=new window.__TAURI__.core.Channel();channel.onmessage=async bytes=>{const event={payload:JSON.parse(new TextDecoder().decode(bytes))};
  let error=null;try{if(barrier||actionBusy||!note||note.deleted)throw Error('capacity editor unavailable');if(event.payload.document){const error=input.load({document:event.payload.document,text:event.payload.text});if(error)throw Error(error);}else input.value=event.payload.text;await flush();}catch(e){error=String(e)}
  await invoke('note_capacity_ack',{token:event.payload.token,error,draft:input.value,status:status.textContent});
 };await invoke('note_capacity_subscribe',{channel});}
 if(['notes-fault','notes-restart'].includes(testMode))await listen('note-test-reload',async event=>{await load();await invoke('note_ready');await invoke('note_capacity_ack',{token:event.payload,error:note?null:status.textContent,draft:input.value,status:status.textContent});});
 await load();finishInitialLoad();trace('initial-load-finished');
 await invoke('note_ready');
 if(note)input.focus();
})();
