const invoke=window.__TAURI__.core.invoke;
const id=new URLSearchParams(location.search).get('id');
const $=s=>document.getElementById(s);
async function action(action){await invoke('notes_action',{id,action});}
function fail(error){if(id)$('status').textContent=String(error);else alert(String(error));}
if(id){
 document.body.classList.add('paper');$('note').hidden=false;
 $('text').focus();
 let pending='',writing=false;
 async function save(){if(writing)return;writing=true;try{while(pending!==null){const text=pending;pending=null;await invoke('notes_update',{text});}$('status').textContent='';}catch(e){fail(e);}finally{writing=false;}}
 $('text').addEventListener('input',()=>{pending=$('text').value;save();});
 $('pin').onclick=async()=>{try{await action('pin');const notes=await invoke('notes_list');$('pin').setAttribute('aria-pressed',String(notes.find(n=>n.id===id).pinned));}catch(e){fail(e);}};
 $('hide').onclick=()=>action('hide').catch(fail);
 $('drag').onpointerdown=e=>{if(e.button===0&&!e.target.closest('button'))invoke('notes_drag').catch(fail);};
}else{
 $('home').hidden=false;
 $('create').onclick=()=>invoke('notes_create').then(refresh).catch(fail);
 $('quit').onclick=()=>{if(confirm('结束原型将清空本次便签内容，确定结束？'))invoke('notes_quit');};
 let previous='';
 async function refresh(){try{const notes=await invoke('notes_list'),key=JSON.stringify(notes);if(key===previous)return;previous=key;$('list').replaceChildren();if(!notes.length){const empty=document.createElement('div');empty.className='empty';empty.textContent='这里还没有便签。\n新建一张，就可以开始写。';$('list').append(empty);}for(const note of notes){const row=document.createElement('button');row.className='row';const text=document.createElement('span');text.textContent=note.text.trim().split('\n')[0]||'空白便签';const state=document.createElement('small');state.textContent=note.visible?'桌面上':'已收起';row.append(text,state);row.onclick=()=>invoke('notes_action',{id:note.id,action:'show'}).then(refresh).catch(fail);$('list').append(row);}}catch(e){fail(e);}}
 refresh();setInterval(refresh,700);
}
