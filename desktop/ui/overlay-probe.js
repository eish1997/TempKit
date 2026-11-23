const $=id=>document.getElementById(id),{invoke,Channel}=window.__TAURI__.core;
const panel=new URLSearchParams(location.search).has('panel');let passthrough=false,drag=null;
$('board').hidden=panel;$('panel').hidden=!panel;document.body.classList.toggle('panel',panel);
document.querySelectorAll('[data-icon]').forEach(el=>el.innerHTML=window.quickgenIcons[el.dataset.icon]||'');
function cancelDrag(){if(drag?.el.hasPointerCapture(drag.pointerId))drag.el.releasePointerCapture(drag.pointerId);drag=null}
function showMode(mode){passthrough=mode.passthrough;cancelDrag();document.body.classList.toggle('pass',passthrough);document.querySelectorAll('.peek').forEach(el=>el.classList.remove('peek'));$('state').textContent=(passthrough?'桌面穿透':'画板操作')+' · Alt＋· 切换';$('pin').setAttribute('aria-pressed',String(mode.pinned))}
function placePicture(el,x,y){const r=el.getBoundingClientRect(),p=window.overlayLayout.clampPicture(x,y,r.width,r.height,innerWidth,innerHeight);el.style.left=p.x+'px';el.style.top=p.y+'px'}
function fitVisible(){cancelDrag();document.querySelectorAll('.picture').forEach(el=>{const r=el.getBoundingClientRect();placePicture(el,r.left,r.top);el.classList.remove('peek')})}
window.addEventListener('resize',fitVisible);window.addEventListener('blur',cancelDrag);
function report(error){$('error').textContent=String(error)}
async function command(name){try{showMode(await invoke(name))}catch(error){report(error)}}
$('mode').onclick=()=>command('probe_toggle');$('pin').onclick=()=>command('probe_pin');$('quit').onclick=()=>invoke('probe_quit');
async function start(){await window.__TAURI__.event.listen('probe-mode',e=>showMode(e.payload));await window.__TAURI__.event.listen('probe-error',e=>report(e.payload));await window.__TAURI__.event.listen('probe-display-changed',fitVisible);showMode(await invoke('probe_state'));if(!panel){fitVisible();const channel=new Channel();channel.onmessage=point=>{document.querySelectorAll('.picture').forEach(el=>{const r=el.getBoundingClientRect();el.classList.toggle('peek',passthrough&&point.x>=r.left&&point.x<=r.right&&point.y>=r.top&&point.y<=r.bottom)})};await invoke('probe_subscribe',{channel})}}
document.querySelectorAll('.picture').forEach(el=>{el.onpointerdown=e=>{if(passthrough||e.button!==0)return;const r=el.getBoundingClientRect();drag={el,pointerId:e.pointerId,x:e.clientX,y:e.clientY,left:r.left,top:r.top};el.setPointerCapture(e.pointerId)};el.onpointermove=e=>{if(drag?.el!==el)return;placePicture(el,drag.left+e.clientX-drag.x,drag.top+e.clientY-drag.y)};el.onpointerup=el.onpointercancel=cancelDrag});
start().catch(report);
