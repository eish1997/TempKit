let saveTimer,loaded=false,saveChain=Promise.resolve();
function saveLayout(){clearTimeout(saveTimer);const value={...state,heightIndex};saveChain=saveChain.catch(()=>{}).then(()=>invoke('save_config',{section:'layout',value}));return saveChain}
function scheduleSave(){if(!loaded)return;clearTimeout(saveTimer);saveTimer=setTimeout(()=>saveLayout().catch(error),250)}
const invoke=window.__TAURI__.core.invoke;
const traceInput=stage=>invoke('trace_input',{stage}).catch(()=>{});
const $=id=>document.getElementById(id);
const icon=(id,name)=>$(id).innerHTML=quickgenIcons[name];
icon('entry','panel-top');icon('height','arrow-up-down');icon('close','x');
let state={edge:'top',fraction:.5,width:280,height:220}, layout, current, opened=false, generation=0, pointer=null, configuring=false, queued=false;
const reduced=matchMedia('(prefers-reduced-motion: reduce)');
const cardDropGate=quickgenInput.createCardDropGate();
function error(e){$('error').textContent=String(e)}
function full(){return {x:0,y:0,width:layout.width,height:layout.height}}
let paintChain=Promise.resolve();
function paint(rect,id=generation){
 const task=paintChain.catch(()=>{}).then(async()=>{
  if(id!==generation)return;
  await invoke('shape',rect);current=rect;
  Object.assign($('surface').style,{left:rect.x+'px',top:rect.y+'px',width:rect.width+'px',height:rect.height+'px'});
 });paintChain=task;return task;
}
async function configure(){if(configuring){queued=true;return}configuring=true;try{generation++;const requested={...state};layout=await invoke('configure',requested);if(state.width===requested.width)state.width=layout.width;if(state.height===requested.height)state.height=layout.height;Object.assign($('entry').style,{left:layout.entry.x+'px',top:layout.entry.y+'px',width:layout.entry.width+'px',height:layout.entry.height+'px'});await paint(opened?full():layout.entry);if(opened){document.body.classList.add('open');cardDropGate.expanded()}scheduleSave();return true}catch(e){error(e);return false}finally{configuring=false;if(queued){queued=false;configure()}}}
async function toggle(value){if(!layout||value===opened)return;opened=value;cardDropGate.reset();const id=++generation;
$('entry').hidden=opened;$('entry').style.opacity='0';$('panel').inert=!opened;
if(!opened){document.body.classList.remove('open');closeManager()}
try{
 // Finish an in-flight native region update before taking the reversal's start rectangle.
 await paintChain;if(id!==generation)return;
 if(reduced.matches&&!opened){await new Promise(resolve=>setTimeout(resolve,100));if(id!==generation)return}
 const from={...current},to=opened?full():layout.entry,start=performance.now(),duration=reduced.matches?0:280;
 while(id===generation){let t=duration?Math.min(1,(performance.now()-start)/duration):1;let eased=1-Math.pow(1-t,3);let rect={};for(const key of ['x','y','width','height'])rect[key]=from[key]+(to[key]-from[key])*eased;await paint(rect,id);if(id!==generation)return;if(opened&&t>.3)document.body.classList.add('open');if(t===1)break;await new Promise(requestAnimationFrame)}
 if(id===generation&&!opened)$('entry').style.opacity='1';
 if(id===generation&&opened)cardDropGate.expanded();
}catch(e){error(e)}}
$('entry').onclick=()=>{if(!pointer?.moved)toggle(true)};$('close').onclick=()=>toggle(false);
for(const dir of ['n','s','e','w','ne','nw','se','sw']){const h=document.createElement('button');h.className='resize '+dir;h.dataset.resize=dir;h.setAttribute('aria-label','调整 '+dir+' 边');$('panel').append(h);h.onkeydown=e=>{if(!e.key.startsWith('Arrow'))return;e.preventDefault();state.width=Math.max(120,state.width+(e.key==='ArrowRight'?16:e.key==='ArrowLeft'?-16:0));state.height=Math.max(120,state.height+(e.key==='ArrowDown'?16:e.key==='ArrowUp'?-16:0));configure()}}
document.addEventListener('pointerdown',e=>{if(e.button!==0)return;const target=e.target;const dir=target.dataset.resize;const move=target.id==='entry'||target.id==='space'||target.tagName==='HEADER';if(!dir&&!move)return;pointer={x:e.screenX,y:e.screenY,width:state.width,height:state.height,dir,moved:false};target.setPointerCapture(e.pointerId)});
document.addEventListener('pointermove',e=>{if(!pointer||!layout)return;const dx=e.screenX-pointer.x,dy=e.screenY-pointer.y;if(!pointer.moved&&Math.hypot(dx,dy)<5)return;pointer.moved=true;if(pointer.dir){state.width=Math.max(120,pointer.width+(pointer.dir.includes('e')?dx:pointer.dir.includes('w')?-dx:0));state.height=Math.max(120,pointer.height+(pointer.dir.includes('s')?dy:pointer.dir.includes('n')?-dy:0))}else{const a=layout.area,x=e.screenX-a.x,y=e.screenY-a.y;state.edge=y<Math.min(x,a.width-x)?'top':x<a.width/2?'left':'right';state.fraction=state.edge==='top'?x/a.width:y/a.height}configure()});
document.addEventListener('pointerup',()=>{scheduleSave();setTimeout(()=>pointer=null,0)});document.addEventListener('pointercancel',()=>pointer=null);
let heights=[32,56,80,112],heightIndex=2,dragHeight=null;
function setHeight(i){heightIndex=Math.max(0,Math.min(3,i));document.documentElement.style.setProperty('--card-height',heights[heightIndex]+'px');$('height').setAttribute('aria-valuenow',heightIndex);scheduleSave()}
$('height').setAttribute('role','slider');$('height').setAttribute('aria-valuemin','0');$('height').setAttribute('aria-valuemax','3');$('height').setAttribute('aria-orientation','vertical');
const heightGlyph=$('height').querySelector('svg');let heightFrame=0,heightOffset=0,heightVelocity=0;
function drawHeight(){heightGlyph.style.transform=`translateY(${heightOffset}px)`}
function releaseHeight(){
 if(!dragHeight)return;dragHeight=null;cancelAnimationFrame(heightFrame);
 if(reduced.matches){heightOffset=heightVelocity=0;drawHeight();return}
 let previous=performance.now();
 function tick(now){const dt=Math.min((now-previous)/1000,1/60);previous=now;
  // mass 1, stiffness 100, damping 10; keep the displacement within the icon's row.
  heightVelocity+=(-100*heightOffset-10*heightVelocity)*dt;heightOffset+=heightVelocity*dt;
  if(Math.abs(heightOffset)<.05&&Math.abs(heightVelocity)<.1){heightOffset=heightVelocity=0;drawHeight();return}
  drawHeight();heightFrame=requestAnimationFrame(tick);
 }heightFrame=requestAnimationFrame(tick);
}
$('height').onpointerdown=e=>{if(e.button!==0)return;cancelAnimationFrame(heightFrame);dragHeight={y:e.clientY,index:heightIndex,offset:heightOffset};e.currentTarget.setPointerCapture(e.pointerId)};
$('height').onpointermove=e=>{if(!dragHeight)return;const dy=e.clientY-dragHeight.y;setHeight(dragHeight.index+Math.round(dy/22));heightOffset=reduced.matches?0:Math.max(-3,Math.min(3,dragHeight.offset+dy*.15));heightVelocity=0;drawHeight()};
$('height').onpointerup=$('height').onpointercancel=$('height').onlostpointercapture=releaseHeight;
$('height').onkeydown=e=>{if(!['ArrowUp','ArrowDown','Home','End'].includes(e.key))return;e.preventDefault();setHeight(e.key==='Home'?0:e.key==='End'?3:heightIndex+(e.key==='ArrowDown'?1:-1))};
setHeight(2);
let pendingImage=null,fileSequence=0,entryDrag=null,triggering=false;
icon('pending-dismiss','x');
function hidePending(){pendingImage=null;$('pending').hidden=true;$('pending-preview').removeAttribute('src')}
$('pending').onclick=()=>{
 ++fileSequence;hidePending();clearHit();$('error').textContent='';
 if(document.activeElement===$('pending'))$('height').focus({preventScroll:true});
};
const dragHover=quickgenInput.createHoverIntent(()=>{if(!opened&&entryDrag)toggle(true)});
const validPath=quickgenInput.acceptsPath;
function hitCard(position){const scale=window.devicePixelRatio||1;const x=position.x/scale,y=position.y/scale;return {card:document.elementFromPoint(x,y)?.closest('.card'),x,y}}
function clearHit(){document.querySelectorAll('.drop-target').forEach(el=>el.classList.remove('drop-target'))}
async function trigger(card){if(!pendingImage||triggering)return;triggering=true;const input=pendingImage;try{await invoke('record_trigger',{path:input.path,ability:card.textContent.trim()});if(pendingImage===input)hidePending();$('error').textContent=card.textContent.trim()+' · 已记录模拟触发'}catch(e){error(e)}finally{triggering=false}}
$('cards').addEventListener('click',e=>{const card=e.target.closest('.card');if(card)trigger(card)});
for(const card of document.querySelectorAll('.card')){card.tabIndex=0;card.setAttribute('role','button');card.onkeydown=e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();trigger(card)}}}
const inputChannel=new window.__TAURI__.core.Channel();
inputChannel.onmessage=async payload=>{
 if(payload.type==='ready'){traceInput('bridge-ready');return}
 if(['enter','drop','leave'].includes(payload.type))traceInput('js-'+payload.type);
 if(payload.type==='leave'){dragHover.cancel();clearHit();if(!opened)entryDrag=null;return} // After expansion, retain the origin guard across region-morph Leave events.
 const hit=hitCard(payload.position);
 if(payload.type==='enter'){
  // Some drag sources reveal their file path only on release.
  // Reject known unsupported/multiple files, but preserve unknown hover intent.
  if(payload.paths.length>1||(payload.paths[0]&&!validPath(payload.paths[0])))return;
  if(!opened){entryDrag={x:hit.x,y:hit.y,rect:{...layout.entry}};dragHover.start()}
 }
 if(payload.type==='over'||payload.type==='enter'){
  clearHit();const r=entryDrag?.rect;
  const moved=cardDropGate.ready()&&quickgenInput.intentionalCardDrop(entryDrag,hit);
  if(moved)hit.card?.classList.add('drop-target');return;
 }
 if(payload.type!=='drop')return;
 dragHover.cancel();
 const entry=entryDrag;entryDrag=null;clearHit();
 // Decide at release time. Slow file decoding must not turn an early drop into a trigger.
 const mayTrigger=opened&&cardDropGate.ready()&&quickgenInput.intentionalCardDrop(entry,hit);
 const seq=++fileSequence;
 if(payload.paths.length!==1){error('请一次拖入一张图片');return}
 try{
  const image=await invoke('read_image',{path:payload.paths[0]});
  traceInput('read-ok');
  const decoded=new Image();decoded.src=image.data;await decoded.decode();
  traceInput('decode-ok');
  if(seq!==fileSequence)return;
  pendingImage=image;$('pending-preview').src=image.data;$('pending').hidden=false;$('error').textContent='';if(!opened)await toggle(true);
  if(hit.card&&mayTrigger)await trigger(hit.card);
 }catch(e){traceInput('drop-error');if(seq===fileSequence){if(!opened)await toggle(true);error(e instanceof DOMException?'图片内容损坏或格式不支持':e)}}
};
invoke('subscribe_input',{channel:inputChannel}).then(()=>traceInput('listener-ready')).catch(e=>{traceInput('listener-error');error(e)});

icon('add','plus');icon('library','library');
let openingManager=false,managerGeneration=0;
function closeManager(){++managerGeneration;return invoke('close_manager').catch(error)}
async function openManager(mode){
 if(!opened||openingManager)return;
 openingManager=true;const request=managerGeneration;
 try{
  // Native creation can finish after the main panel has already collapsed.
  // Prepare hidden, then show only while this opening request remains current.
  await invoke('open_manager',{mode});
  if(opened&&request===managerGeneration)await invoke('show_manager');
 }catch(e){if(request===managerGeneration)error(e)}finally{openingManager=false}
}
$('add').onclick=()=>openManager('add');$('library').onclick=()=>openManager('library');
let abilityRenderSequence=0;
async function renderAbilities(){const sequence=++abilityRenderSequence;const cfg=await invoke('load_config');if(sequence!==abilityRenderSequence)return;$('cards').replaceChildren();for(const ability of cfg.abilities){const card=document.createElement('button');card.className='card';card.dataset.ability=ability.id;const label=document.createElement('span');label.textContent=ability.name;card.append(label);const img=document.createElement('img');const fallback='covers/'+(['clay','material','character','mood','expand','style'].includes(ability.id)?ability.id:'clay')+'.svg';img.src=ability.cover||fallback;img.alt='';img.onerror=()=>{img.onerror=null;img.src=fallback};card.prepend(img);$('cards').append(card)}}
(async()=>{try{const cfg=await invoke('load_config');state={...state,...cfg.layout};setHeight(cfg.layout.heightIndex);await renderAbilities();loaded=true;await configure();if(cfg.recoveryWarning){await toggle(true);error(cfg.recoveryWarning)}await window.__TAURI__.event.listen('config-changed',e=>{if(e.payload==='abilities')renderAbilities().catch(error)})}catch(e){error(e);await configure()}})();

window.__TAURI__.window.getCurrentWindow().onCloseRequested(async event=>{event.preventDefault();try{if(loaded)await saveLayout();await invoke('quit')}catch(e){error(e)}}).catch(error);


// Metadata changes cover DPI, monitor removal and taskbar work-area changes.
const checkDisplay=createDisplayWatch(()=>invoke('display_snapshot'),async()=>{if(!loaded||configuring)throw Error('Layout not ready');pointer=null;if(await configure()===false)throw Error('Layout refresh failed')});
checkDisplay().catch(()=>{});
setInterval(()=>checkDisplay().catch(()=>{}),2000);
