// Shared by both native webviews; the host persists and broadcasts preferences.
(()=>{
 const modes=['system','light','dark'],names={system:'跟随系统',light:'浅色',dark:'深色'},icons={system:'monitor',light:'sun',dark:'moon'};
 const root=document.documentElement,button=document.getElementById('theme');
 const invoke=window.__TAURI__.core.invoke;
 let mode='system',sequence=0,busy=false;
 function apply(value){
  mode=modes.includes(value)?value:'system';root.dataset.theme=mode;
  try{localStorage.setItem('quickgen-theme',mode)}catch{}
  if(button){button.innerHTML=window.quickgenIcons[icons[mode]];button.setAttribute('aria-label','主题：'+names[mode]+'；切换到'+names[modes[(modes.indexOf(mode)+1)%3]])}
 }
 function fail(e){const target=document.getElementById('error');if(target)target.textContent=String(e)}
 async function refresh(){const id=++sequence;const cfg=await invoke('load_config');if(id===sequence)apply(cfg.theme)}
 try{apply(localStorage.getItem('quickgen-theme'))}catch{apply('system')}
 if(button)button.onclick=async()=>{
  if(busy)return;busy=true;button.disabled=true;++sequence;
  const previous=mode,next=modes[(modes.indexOf(mode)+1)%3];apply(next);
  try{await invoke('save_config',{section:'theme',value:next})}catch(e){++sequence;apply(previous);fail(e)}
  finally{busy=false;button.disabled=false}
 };
 // Subscribe before the initial read so an already-open utility panel updates too.
 window.__TAURI__.event.listen('config-changed',e=>{if(e.payload==='theme')refresh().catch(fail)}).then(()=>refresh()).catch(fail);
})();
