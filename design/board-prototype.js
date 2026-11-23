(()=>{
const $=id=>document.getElementById(id),icon=name=>quickgenIcons[name]||quickgenIcons['plus'];
document.querySelectorAll('[data-icon]').forEach(el=>el.innerHTML=icon(el.dataset.icon));
const presets=[['clay','白模转写实'],['material','材质替换'],['character','角色精修'],['mood','场景氛围'],['expand','图片扩展'],['style','风格迁移']];
const src=id=>'../desktop/ui/covers/'+id+'.svg';
const items=[['clay','体块研究'],['material','材质构成'],['mood','场景气氛'],['character','角色草稿']].map(([id,name],index)=>({id,name,src:src(id),x:0,y:0,addedAt:Date.now()-(4-index)*3600000,order:index}));
let open=false,pinned=false,placed=false,scale=1,tx=0,ty=0,selected=null,gesture=null,noticeTimer,ghost=null,nextId=1;
let passthrough=false;
let storageReady=false,saveTimer,db;
const jobs=[];const localUrls=new Set();let jobId=0;
const selection=new Set();let spaceHeld=false;
const marquee=document.createElement("div");marquee.className="selection-marquee";marquee.hidden=true;document.body.append(marquee);
const reduced=matchMedia('(prefers-reduced-motion: reduce)').matches;
let stackHovered=null,suppressStackClick=false;
const cards=new Map();
for(const [id,name]of presets){const card=document.createElement('div');card.className='ability';card.dataset.preset=id;card.setAttribute('aria-label',name+'：拖入图片执行');card.innerHTML='<img alt="" src="'+src(id)+'"><span>'+name+'</span>';$('abilities').append(card);cards.set(id,{el:card,name})}
function notice(text){clearTimeout(noticeTimer);$('notice').textContent=text;$('notice').hidden=false;noticeTimer=setTimeout(()=>$('notice').hidden=true,2400)}
function camera(){saveSoon(); $('world').style.transform=`translate(${tx}px,${ty}px) scale(${scale})`;$('zoom-value').textContent=Math.round(scale*100)+'%';$('zoom-out').disabled=scale<=.35;$('zoom-in').disabled=scale>=2.5 }
function zoom(next,x=innerWidth/2,y=innerHeight/2){next=Math.max(.35,Math.min(2.5,next));tx=x-(x-tx)*next/scale;ty=y-(y-ty)*next/scale;scale=next;camera()}
function arrange(){const r=$('dock').getBoundingClientRect(),cols=r.left>440?2:1;scale=innerWidth<700?.72:1;tx=0;ty=0;const w=180*scale,h=160*scale,gap=18*scale;const startX=Math.max(16,r.left-80-cols*w-(cols-1)*gap),startY=Math.max(78,r.top-24);items.forEach((item,i)=>{item.x=(startX+(i%cols)*(w+gap))/scale;item.y=(startY+Math.floor(i/cols)*h)/scale});placed=true;renderImages();camera()}
function renderStack(){saveSoon(); $('stack-pictures').replaceChildren();items.slice(-4).forEach((item,index,visible)=>{const img=document.createElement('img');img.className='stack-picture';const offset=index-(visible.length-1)/2;img.dataset.image=item.id;img.draggable=false;img.style.setProperty('--order',index);img.style.setProperty('--x',offset*Math.min(22,100/Math.max(1,visible.length-1))+'px');img.style.setProperty('--y',Math.abs(offset)*3-5+'px');img.style.setProperty('--angle',offset*8+'deg');img.src=item.src;img.alt='';$('stack-pictures').append(img)});$('stack').classList.toggle('empty',!items.length);$('stack').setAttribute('aria-label','打开图片画板，'+items.length+'张图片') }
function hoverStack(e){
 if(gesture)return;const pictures=[...$('stack-pictures').children];if(!pictures.length)return;
 const r=$('stack').getBoundingClientRect(),ratio=Math.max(0,Math.min(.999,(e.clientX-r.left)/r.width));
 const index=Math.floor(ratio*pictures.length);stackHovered=pictures[index].dataset.image;
 pictures.forEach((n,i)=>n.classList.toggle('lifted',i===index));
}
$('stack').onpointermove=hoverStack;
$('stack').onpointerleave=()=>{if(gesture?.source==='stack')return;stackHovered=null;document.querySelectorAll('.stack-picture').forEach(n=>n.classList.remove('lifted'))};
$('stack').onpointerdown=e=>{if(e.button!==0)return;hoverStack(e);const item=items.find(i=>i.id===stackHovered);if(!item)return;if(!placed)arrange();startDrag(e,item,$('stack'),'stack')};
function renderImages(){ $('world').replaceChildren();items.forEach(item=>{const el=document.createElement('div');el.className='board-image'+(item.result?' simulated-result':'');el.tabIndex=0;el.dataset.image=item.id;el.setAttribute('aria-label',item.name+'，可拖动到能力卡片');el.style.left=item.x+'px';el.style.top=item.y+'px';const figure=document.createElement('figure'),img=document.createElement('img'),caption=document.createElement('figcaption'),remove=document.createElement('button');img.src=item.src;img.alt=item.name;img.draggable=false;caption.textContent=item.name;remove.className='remove';remove.setAttribute('aria-label','移除'+item.name);remove.innerHTML=icon('x');remove.onclick=e=>{e.stopPropagation();const index=items.indexOf(item);if(index>=0)items.splice(index,1);selection.delete(item.id);if(selected===item.id)selected=[...selection].at(-1)||null;renderImages();renderStack();renderOutline()};figure.append(img,remove);el.append(figure,caption);el.onpointerdown=e=>startDrag(e,item,el);el.onfocus=()=>{if(!selection.has(item.id))selectImage(item.id)};el.classList.toggle('selected',selection.has(item.id));el.onkeydown=e=>{if(['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(e.key)){e.preventDefault();item.x+=(e.key==='ArrowRight'?12:e.key==='ArrowLeft'?-12:0);item.y+=(e.key==='ArrowDown'?12:e.key==='ArrowUp'?-12:0);el.style.left=item.x+'px';el.style.top=item.y+'px'}};$('world').append(el)});renderJobs() }
function centerOutline(animate=false){
 if(!open||!selected)return;const rail=$('outline'),thumb=[...rail.querySelectorAll('.outline-thumb')].find(n=>n.dataset.image===selected);if(!thumb)return;
 rail.style.setProperty('--edge-pad',Math.max(0,(rail.clientHeight-34)/2)+'px');
 const r=rail.getBoundingClientRect(),t=thumb.getBoundingClientRect();rail.scrollTo({top:rail.scrollTop+t.top+t.height/2-r.top-r.height/2,behavior:animate&&!reduced?'smooth':'instant'});
}
function paintSelection(){document.querySelectorAll('.board-image,.outline-thumb').forEach(n=>{const active=selection.has(n.dataset.image);n.classList.toggle('selected',active);if(n.classList.contains('outline-thumb'))n.setAttribute('aria-pressed',String(active))})}
function selectImage(id,center=true){selected=id;selection.clear();if(id)selection.add(id);paintSelection();if(center)centerOutline(true)}
function renderOutline(){const list=$('outline-list');list.replaceChildren();[...items].sort((a,b)=>b.addedAt-a.addedAt||b.order-a.order).forEach(item=>{const thumb=document.createElement('button');thumb.className='outline-thumb';thumb.dataset.image=item.id;thumb.dataset.addedAt=String(item.addedAt);thumb.setAttribute('aria-label','大纲：'+item.name);thumb.setAttribute('aria-pressed',String(selected===item.id));const img=document.createElement('img');img.src=item.src;img.alt='';img.draggable=false;thumb.append(img);thumb.onpointerdown=e=>startDrag(e,item,thumb,'outline');thumb.onclick=()=>selectImage(item.id);list.append(thumb)});if(!selected){$('outline').style.setProperty('--edge-pad','0px');$('outline').scrollTop=0}paintSelection();centerOutline()}
new ResizeObserver(()=>{if(open&&!gesture)centerOutline()}).observe($('panel'));
function showCanvas(){if(!items.length){$('files').click();return}open=true;$('canvas').hidden=false;$('canvas-tools').hidden=false;$('outline').hidden=false;$('stack').setAttribute('aria-expanded','true');if(!placed)arrange();renderOutline();if(!reduced)$('canvas').animate([{opacity:0},{opacity:1}],{duration:180,easing:'ease-out'});camera()}
function hideCanvas(){setPassThrough(false);cancelGesture();open=false;$('canvas').hidden=true;$('canvas-tools').hidden=true;$('outline').hidden=true;$('stack').setAttribute('aria-expanded','false')}
function setPassThrough(value){
 clearDesktopPeek();
 passthrough=Boolean(value&&open);cancelGesture();spaceHeld=false;$('canvas').classList.remove('pan-ready');document.body.classList.toggle('desktop-mode',passthrough);$('canvas').inert=passthrough;$('outline').inert=passthrough;
 if(!passthrough&&$('desktop').contains(document.activeElement))$('canvas-mode').focus();
 $('canvas-mode').setAttribute('aria-pressed',String(passthrough));$('canvas-mode').setAttribute('aria-label',passthrough?'恢复画板操作模式':'切换桌面穿透模式');$('canvas-mode').innerHTML=icon(passthrough?'panel-top':'monitor');
 $('outline').hidden=passthrough||!open;$('dock').hidden=passthrough&&!pinned;$('notch').hidden=!$('dock').hidden;
 saveSoon();
}
$('canvas-mode').onclick=()=>setPassThrough(!passthrough);
function clearDesktopPeek(){document.querySelectorAll('.desktop-peek').forEach(el=>el.classList.remove('desktop-peek'))}
document.addEventListener('pointermove',e=>{
 if(!passthrough)return;
 document.querySelectorAll('#world .board-image').forEach(el=>{const rect=el.getBoundingClientRect();el.classList.toggle('desktop-peek',e.clientX>=rect.left&&e.clientX<=rect.right&&e.clientY>=rect.top&&e.clientY<=rect.bottom)});
});
document.documentElement.addEventListener('pointerleave',clearDesktopPeek);
window.addEventListener('blur',clearDesktopPeek);
document.addEventListener('keydown',e=>{if(e.code==='Backquote'&&e.altKey&&!e.ctrlKey&&!e.metaKey&&!e.shiftKey){e.preventDefault();e.stopImmediatePropagation();if(open&&!e.repeat)setPassThrough(!passthrough)}},true);
document.querySelectorAll('[data-desktop-app]').forEach(button=>button.onclick=()=>{
 $('desktop-window').hidden=false;$('desktop-title').textContent=button.dataset.desktopApp;const content=$('desktop-content');content.replaceChildren();
 if(button.dataset.desktopApp==='便签'){const input=document.createElement('textarea');input.placeholder='在这里输入，测试桌面操作';input.setAttribute('aria-label','模拟桌面便签');content.append(input)}
 else if(button.dataset.desktopApp==='素材文件夹'){for(const name of ['参考图片','设计稿','导出文件']){const row=document.createElement('button');row.textContent=name;row.onclick=()=>{row.textContent=name+' · 已打开'};content.append(row)}}
 else{const image=document.createElement('img');image.src=src('clay');image.alt='模拟场景预览';content.append(image);const toggle=document.createElement('button');toggle.textContent='切换场景';toggle.onclick=()=>{image.src=image.src.endsWith('clay.svg')?src('mood'):src('clay')};content.append(toggle)}
});$('desktop-close').onclick=()=>$('desktop-window').hidden=true;
function restorePanel(){ $('dock').hidden=false;$('notch').hidden=true }
function collapse(){hideCanvas();$('dock').hidden=true;$('notch').hidden=false}
function targetAt(x,y){return [...cards.values()].find(card=>{const r=card.el.getBoundingClientRect();return x>=r.left&&x<=r.right&&y>=r.top&&y<=r.bottom})}
function highlight(card){cards.forEach(c=>c.el.classList.toggle('target',c===card))}
function freeResultPosition(input){
 const occupied=[...items,...jobs.filter(j=>j.state!=='done')];
 for(let ring=1;ring<60;ring++)for(const [dx,dy] of [[-198*ring,0],[198*ring,0],[0,160*ring],[0,-160*ring]]){
 const x=input.x+dx,y=input.y+dy;const panel=$('dock').getBoundingClientRect();if(x*scale+tx+180*scale>panel.left-64&&x*scale+tx<panel.right&&y*scale+ty+144*scale>panel.top-80&&y*scale+ty<panel.bottom+40)continue;
 if(!occupied.some(i=>Math.abs(i.x-x)<190&&Math.abs(i.y-y)<150))return {x,y};
 }return {x:input.x-198,y:input.y};
}
function renderJobs(){saveSoon();
 document.querySelectorAll('.generation-job').forEach(n=>n.remove());
 for(const j of jobs.filter(j=>j.state!=='done')){
 const el=document.createElement('div');el.className='generation-job '+j.state;el.dataset.job=j.id;el.style.left=j.x+'px';el.style.top=j.y+'px';el.setAttribute('role','status');
 const name=document.createElement('strong');name.textContent=j.input.name+' · '+j.card.name;
 const status=document.createElement('span');status.textContent=({failed:'模拟失败',interrupted:'已中断',cancelled:'已取消',running:'模拟生成中…',queued:'等待生成'})[j.state];el.append(name,status);
 if(['failed','interrupted','cancelled'].includes(j.state)){const retry=document.createElement('button');retry.textContent='重试';retry.setAttribute('aria-label','重试 '+j.input.name);retry.onclick=()=>{j.state='queued';j.fail=false;renderJobs();pumpJobs()};el.append(retry)}
 const action=document.createElement('button');const active=['queued','running'].includes(j.state);action.textContent=active?'取消':'移除';action.setAttribute('aria-label',action.textContent+'任务 '+j.input.name);action.onclick=()=>{if(active){j.state='cancelled';j.attempt=(j.attempt||0)+1}else jobs.splice(jobs.indexOf(j),1);renderJobs();pumpJobs()};el.append(action);
 el.onpointerdown=e=>e.stopPropagation();$('world').append(el);
 }
 renderReceipt();
}
let panelNotice=null;
function renderReceipt(){
 const receipt=$('receipt');receipt.querySelector('[data-notice-action]')?.remove();$('receipt-image').hidden=!!panelNotice;receipt.querySelector('span').hidden=!!panelNotice;
 if(panelNotice){receipt.hidden=false;$('receipt-name').textContent=panelNotice.text;if(panelNotice.undo){const action=document.createElement('button');action.dataset.noticeAction='';action.textContent='撤销';action.style.width='auto';action.onclick=()=>{const undo=panelNotice.undo;panelNotice=null;undo();renderReceipt();};receipt.insertBefore(action,$('receipt-close'));}return;}
 const pending=jobs.filter(j=>j.state==='running'||j.state==='queued').length,failed=jobs.filter(j=>j.state==='failed').length;
 if(!jobs.length)$('receipt').hidden=true;
 if(jobs.length){$('receipt').hidden=false;$('receipt-name').textContent=pending?pending+' 张处理中':failed?failed+' 张失败':jobs.some(j=>j.state!=='done')?'有未完成任务':'全部完成';$('receipt').querySelector('span').textContent='逐张模拟 · '+jobs.filter(j=>j.state==='done').length+' 张完成';$('receipt-image').src=jobs.at(-1).input.src}
}
window.addEventListener('quickgen-panel-notice',e=>{panelNotice=e.detail;renderReceipt();});

function pumpJobs(){
 if(jobs.some(j=>j.state==='running'))return;const j=jobs.find(j=>j.state==='queued');if(!j)return;
 j.state='running';const attempt=j.attempt=(j.attempt||0)+1;renderJobs();setTimeout(()=>finishJob(j,attempt),1800);
}
function finishJob(j,attempt){if(j.state!=='running'||j.attempt!==attempt||!jobs.includes(j))return;
 // Do not replace DOM nodes underneath an active pointer capture.
 if(gesture){setTimeout(()=>finishJob(j,attempt),100);return}
 if(j.fail){j.state='failed';renderJobs();pumpJobs();return}
 j.state='done';const result={id:'result-'+j.id,name:j.input.name+' · '+j.card.name+'（模拟结果）',src:j.input.src,result:true,local:j.input.local,x:j.x,y:j.y,addedAt:Date.now(),order:nextId++};items.push(result);
 renderImages();renderStack();renderOutline();pumpJobs();
}
function simulate(inputs,card){
 const failFirst=$('simulate-failure').checked;$('simulate-failure').checked=false;
 inputs.forEach((input,index)=>{const position=freeResultPosition(input);jobs.push({id:++jobId,input:{...input},card,state:'queued',fail:failFirst&&index===0,...position})});
 renderJobs();pumpJobs();
}
function startDrag(e,item,el,source='canvas'){
 if(e.button!==0||spaceHeld||(source==='canvas'&&e.target.closest('button')))return;e.stopPropagation();e.preventDefault();if(source==='stack')suppressStackClick=false;
 if(source==='canvas'&&selection.has(item.id)){selected=item.id;centerOutline(true)}else selectImage(item.id,source==='canvas');el.focus({preventScroll:true});el.setPointerCapture(e.pointerId);
 gesture={type:'image',source,item,el,id:e.pointerId,x:e.clientX,y:e.clientY,ix:item.x,iy:item.y,moved:false,members:(source==='canvas'?items.filter(i=>selection.has(i.id)):[item]).map(i=>({item:i,x:i.x,y:i.y,el:[...document.querySelectorAll('.board-image')].find(n=>n.dataset.image===i.id)}))};
}
function moveGroup(g,dx=0,dy=0){for(const m of g.members){m.item.x=m.x+dx;m.item.y=m.y+dy;m.el.style.left=m.item.x+'px';m.el.style.top=m.item.y+'px'}}
function cleanDrag(){marquee.hidden=true;if(gesture?.type==='image'){gesture.el.classList.remove('dragging');gesture.el.style.opacity='';gesture.members.forEach(m=>{m.el.classList.remove('dragging');m.el.style.opacity=''})}gesture=null;ghost?.remove();ghost=null;highlight(null);$('canvas').classList.remove('panning')}
function cancelGesture(){if(gesture?.type==='select'){selection.clear();gesture.before.forEach(id=>selection.add(id));selected=gesture.previous;paintSelection();}if(gesture?.type==='image'&&gesture.source==='canvas'){moveGroup(gesture)}cleanDrag()}
 document.addEventListener('pointermove',e=>{
 if(!gesture)return;const dx=e.clientX-gesture.x,dy=e.clientY-gesture.y;
 if(gesture.type==='select'){
 const left=Math.min(e.clientX,gesture.x),top=Math.min(e.clientY,gesture.y),right=Math.max(e.clientX,gesture.x),bottom=Math.max(e.clientY,gesture.y);
 if(Math.hypot(dx,dy)<4&&!gesture.moved)return;gesture.moved=true;marquee.hidden=false;Object.assign(marquee.style,{left:left+'px',top:top+'px',width:(right-left)+'px',height:(bottom-top)+'px'});
 selection.clear();if(gesture.additive)gesture.before.forEach(id=>selection.add(id));document.querySelectorAll('.board-image').forEach(n=>{const r=n.querySelector('figure').getBoundingClientRect();if(r.right>left&&r.left<right&&r.bottom>top&&r.top<bottom)selection.add(n.dataset.image)});selected=[...selection].at(-1)||null;paintSelection();return;
 }
 if(gesture.type==='pan'){tx=gesture.tx+dx;ty=gesture.ty+dy;camera();return}
 if(!gesture.moved&&Math.hypot(dx,dy)<4)return;
 if(!gesture.moved){gesture.moved=true;gesture.el.classList.add('dragging');ghost=document.createElement('img');ghost.className='drag-ghost';ghost.src=gesture.item.src;document.body.append(ghost)}
 const target=targetAt(e.clientX,e.clientY);highlight(target);ghost.hidden=gesture.source==='canvas'&&!target;ghost.style.left=e.clientX+'px';ghost.style.top=e.clientY+'px';gesture.el.style.opacity=target?'.35':'';
 if(gesture.source==='canvas'){moveGroup(gesture,dx/scale,dy/scale);gesture.members.forEach(m=>{m.el.classList.add('dragging');m.el.style.opacity=target?'.35':''})}
 });
 document.addEventListener('pointerup',e=>{
 if(!gesture)return;const g=gesture;
 if(g.source==='stack'&&g.moved)suppressStackClick=true;
 if(g.type==='image'){const card=targetAt(e.clientX,e.clientY);if(card&&g.moved){if(g.source==='canvas'){moveGroup(g)}simulate(g.members.map(m=>m.item),card)}}
 cleanDrag();if(g.type==='image'||g.type==='select')centerOutline(true);
 });document.addEventListener('pointercancel',cancelGesture);
$('canvas').onpointerdown=e=>{
 const pan=e.button===1||(e.button===0&&spaceHeld);
 if(!pan&&(e.button!==0||e.target.closest('.board-image,.generation-job')))return;e.preventDefault();
 if(pan){gesture={type:'pan',x:e.clientX,y:e.clientY,tx,ty};$('canvas').classList.add('panning')}
 else{gesture={type:'select',x:e.clientX,y:e.clientY,before:[...selection],previous:selected,additive:e.shiftKey,moved:false};if(!e.shiftKey){selection.clear();selected=null;paintSelection()}const focused=document.activeElement;if(focused?.closest('.board-image'))focused.blur()}
 $('canvas').setPointerCapture(e.pointerId);
};
 document.addEventListener('keydown',e=>{if(e.code==='Space'&&open&&!passthrough&&!e.target.closest('input,textarea')){e.preventDefault();spaceHeld=true;$('canvas').classList.add('pan-ready')}});
 document.addEventListener('keyup',e=>{if(e.code==='Space'){spaceHeld=false;$('canvas').classList.remove('pan-ready')}});
 window.addEventListener('blur',()=>{spaceHeld=false;$('canvas').classList.remove('pan-ready');cancelGesture()});
$('canvas').addEventListener('wheel',e=>{e.preventDefault();if(gesture)return;zoom(scale*Math.exp(-e.deltaY*.0015),e.clientX,e.clientY)},{passive:false});
$('stack').onclick=()=>{if(suppressStackClick){suppressStackClick=false;return}open?hideCanvas():showCanvas()};$('canvas-close').onclick=hideCanvas;$('zoom-in').onclick=()=>zoom(scale*1.2);$('zoom-out').onclick=()=>zoom(scale/1.2);$('zoom-value').onclick=()=>zoom(1);$('fit').onclick=arrange;
$('pin').onclick=()=>{pinned=!pinned;$('pin').setAttribute('aria-pressed',String(pinned));$('pin').setAttribute('aria-label',pinned?'取消常驻面板':'常驻面板');if(passthrough)setPassThrough(true)};$('notch').onclick=()=>{restorePanel();if(passthrough)setPassThrough(false)};
$('theme').onclick=()=>{const light=document.body.classList.toggle('light');$('theme').innerHTML=icon(light?'sun':'moon');$('theme').setAttribute('aria-label',light?'切换深色主题':'切换浅色主题')};
$('desktop').onpointerdown=()=>{if(!pinned&&!open)collapse()};window.addEventListener('blur',()=>{if(!pinned&&!open&&!picking)collapse()});
document.addEventListener('keydown',e=>{if(e.key==='Escape'){if(gesture){cancelGesture();return}if(open){hideCanvas();$('stack').focus()}else if(!pinned)collapse()}});
$('receipt-close').setAttribute('aria-label','关闭提示');$('receipt').setAttribute('role','status');$('receipt-close').onclick=()=>{if(panelNotice){panelNotice=null;renderReceipt();}else $('receipt').hidden=true;};
let picking=false;$('import').onclick=()=>{picking=true;$('files').click()};$('files').onchange=async e=>{picking=false;await importFiles([...e.target.files]);e.target.value=''};$('files').addEventListener('cancel',()=>picking=false);
async function importFiles(files){let added=0;for(const file of files){if(!/^image\/(png|jpeg|webp)$/.test(file.type)||file.size>20*1024*1024){notice('请选择20 MB以内的PNG、JPG或WebP');continue}const url=await new Promise((resolve,reject)=>{const reader=new FileReader();reader.onload=()=>resolve(reader.result);reader.onerror=reject;reader.readAsDataURL(file)}),img=new Image();img.src=url;try{await img.decode()}catch{URL.revokeObjectURL(url);notice('图片无法读取');continue}const r=$('dock').getBoundingClientRect(),i=items.length;items.push({id:'local-'+nextId++,name:file.name,src:url,local:true,addedAt:Date.now(),order:nextId,x:(Math.max(16,r.left-410)+(i%2)*198-tx)/scale,y:(Math.max(80,r.top-24)+Math.floor(i/2)*160-ty)/scale});added++}if(added){restorePanel();renderStack();renderImages();renderOutline();if(!open)showCanvas()}}
document.addEventListener('dragover',e=>{if([...e.dataTransfer.types].includes('Files')){e.preventDefault();document.body.classList.add('file-over')}});document.addEventListener('dragleave',e=>{if(!e.relatedTarget)document.body.classList.remove('file-over')});document.addEventListener('drop',async e=>{if(!e.dataTransfer.files.length)return;e.preventDefault();document.body.classList.remove('file-over');await importFiles([...e.dataTransfer.files])});
window.addEventListener('resize',()=>{if(open&&placed){camera()}});window.addEventListener('beforeunload',()=>localUrls.forEach(url=>URL.revokeObjectURL(url)));
async function initStorage(){
 document.body.inert=true;
 try{db=await new Promise((resolve,reject)=>{const r=indexedDB.open('quickgen-board-prototype',1);r.onupgradeneeded=()=>r.result.createObjectStore('scene');r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(r.error)});
 const state=await new Promise((resolve,reject)=>{const r=db.transaction('scene').objectStore('scene').get('current');r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(r.error)});
 if(state?.version===1){items.splice(0,items.length,...state.items);jobs.splice(0,jobs.length,...state.jobs.map(j=>({...j,card:cards.get(j.preset),state:['queued','running'].includes(j.state)?'interrupted':j.state})).filter(j=>j.card));scale=state.scale;tx=state.tx;ty=state.ty;placed=state.placed;nextId=state.nextId;jobId=state.jobId;pinned=state.pinned;selection.clear();state.selection.forEach(id=>{if(items.some(i=>i.id===id))selection.add(id)});selected=state.selected;document.body.classList.toggle('light',state.light);$('pin').setAttribute('aria-pressed',String(pinned));if(state.open&&items.length){showCanvas();setPassThrough(state.passthrough)}}
 storageReady=true;
 }catch(error){notice('无法恢复本地画板，请保留当前页面');console.error(error)}
 finally{document.body.inert=false;renderStack();renderImages();renderOutline();camera()}
}
function saveSoon(){if(!storageReady)return;clearTimeout(saveTimer);saveTimer=setTimeout(saveScene,150)}
function saveScene(){if(!db||!storageReady)return;clearTimeout(saveTimer);const state={version:1,items:items.map(i=>({...i})),jobs:jobs.map(({card,...j})=>({...j,preset:card.el.dataset.preset})),scale,tx,ty,placed,nextId,jobId,pinned,selected,selection:[...selection],open,passthrough,light:document.body.classList.contains('light')};try{const t=db.transaction('scene','readwrite');t.objectStore('scene').put(state,'current');t.onerror=()=>notice('本地保存失败，请勿刷新页面')}catch{notice('本地保存失败，请勿刷新页面')}}
 document.addEventListener('pointerup',saveSoon);document.addEventListener('keyup',saveSoon);document.addEventListener('click',saveSoon);document.addEventListener('visibilitychange',()=>{if(document.hidden)saveScene()});window.addEventListener('pagehide',saveScene);
initStorage();
})();
