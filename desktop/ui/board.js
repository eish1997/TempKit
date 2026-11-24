(async()=>{
const $=id=>document.getElementById(id),icon=name=>quickgenIcons[name]||quickgenIcons['plus'];
document.querySelectorAll('[data-icon]').forEach(el=>el.innerHTML=icon(el.dataset.icon));
const invoke=window.__TAURI__.core.invoke;
const initialConfig=await invoke('load_config');
const diagnostic=await invoke('board_diagnostic').catch(()=> '');
if(diagnostic){const timing=document.createElement('script');timing.src='diagnostic-timing.js';document.head.append(timing)}
const presets=[['clay','白模转写实'],['material','材质替换'],['character','角色精修'],['mood','场景氛围'],['expand','图片扩展'],['style','风格迁移']];
const src=id=>'covers/'+id+'.svg';
const items=[];
let open=false,pinned=false,placed=false,scale=1,tx=0,ty=0,selected=null,gesture=null,noticeTimer,ghost=null,nextId=1;
let passthrough=false,outboundDragging=false;
let desktopPeekIds=new Set();
let storageReady=false,storageError=null,saveTimer;
let durable=false,lastLocalScene=null,saveChain=Promise.resolve(),taskSyncBusy=false;
let renderedImageWork='null';
let noteBarrier=false;let quitting=false;let inkTools=null;const pendingBoardWork=new Set();
function trackBoardWork(work){pendingBoardWork.add(work);work.finally(()=>pendingBoardWork.delete(work)).catch(()=>{});return work;}
const sceneClient=new quickgenDurable.SceneClient(invoke),taskMirror=new quickgenDurable.TaskMirror(invoke),resourceImages=new Map(),placedTaskOutputs=new Set();
async function imageSource(id){if(resourceImages.has(id))return resourceImages.get(id);const {data}=await invoke('resource_read',{id});resourceImages.set(id,data);return data;}
function durableSnapshot(){const state=snapshotScene();state.resourceSchemaVersion=1;state.revision=sceneClient.current?.revision||0;for(const item of state.items)delete item.src;for(const job of state.jobs)delete job.input.src;return state;}
function persistDurable(){
 const captured=durableSnapshot();
 const next=saveChain.then(async()=>{const apply=quickgenDurable.sceneDelta(lastLocalScene||captured,captured);const saved=apply.changed?await sceneClient.mutate(apply):sceneClient.current;lastLocalScene=captured;return saved;});
 saveChain=next.catch(()=>{});return next;
}
async function submitDurable(j){
 await persistDurable();
 const results=await invoke('task_submit',{submissions:[{submission_id:j.submissionId,batch_id:j.batchId||null,preset_id:j.card.el.dataset.preset,resource_id:j.input.resourceId,simulate_failure:!!j.fail}]});
 if(results[0].Err){j.state='failed';j.admissionError=results[0].Err;throw Error(results[0].Err)}
 j.hostTaskId=results[0].Ok.id;taskMirror.apply(results[0].Ok);delete j.admissionError;await persistDurable();
}
function durableAction(j,action){return trackBoardWork(durableActionWork(j,action));}
async function durableActionWork(j,action){try{
 if(!j.hostTaskId){if(action==='retry'){j.submissionId=j.submissionId||crypto.randomUUID();j.fail=false;await submitDurable(j)}else{jobs.splice(jobs.indexOf(j),1);await persistDurable();}return;}
 await taskMirror.sync();
 const task=taskMirror.tasks.get(j.hostTaskId);
 if(action==='retry'&&task?.failure?.outcome_unknown&&task.request.capability.connection_id!=='internal-mock'){
  hints.set('regenerate:'+task.id,{priority:40,text:'原请求可能已执行并计费，重新生成可能再次计费',actionText:'确认重新生成',dismissible:true,action:()=>{hints.clear('regenerate:'+task.id);confirmRegenerate(task);}});return;
 }
 await taskMirror.control(j.hostTaskId,action);await syncDurableTasks();
 }catch(error){notice(String(error));}finally{renderJobs();}}
function confirmRegenerate(task){
 return trackBoardWork(invoke('task_control',{command:{task_id:task.id,attempt:task.attempt,event_version:task.event_version,action:'regenerate'}}).then(async next=>{taskMirror.apply(next);await syncDurableTasks();}).catch(error=>notice(String(error))));
}
async function syncDurableTasks(){
 if(!durable||!storageReady||taskSyncBusy||quitting)return;taskSyncBusy=true;
 try{
  await taskMirror.sync();
  if(taskMirror.imageWork?.save_failed)hints.set('image-result-save',{priority:40,text:'图片结果尚未保存，请保留程序并重试保存',actionText:'重试保存',action:()=>trackBoardWork(taskMirror.retryImageSave().then(()=>syncDurableTasks()).catch(error=>notice('保存未完成：'+String(error))))});else hints.clear('image-result-save');
  if(taskMirror.schedulerError)throw Error('任务暂停：'+taskMirror.schedulerError);
  if(gesture||panelLayout?.active)return;
  let changed=false;
  // Host tasks drive reconciliation; local rows only control presentation.
  for(const task of taskMirror.tasks.values()){
   let j=jobs.find(j=>j.hostTaskId===task.id||j.submissionId===task.request.submission_id);
   if(!j&&task.state!=='succeeded'){
    const input=items.find(i=>i.resourceId===task.request.inputs[0]?.resource_id)||{id:'task-input-'+task.id,resourceId:task.request.inputs[0]?.resource_id,name:'原图',x:0,y:0};
    const card=cards.get(task.request.preset_id)||{name:'已移除能力',prompt:'',el:{dataset:{preset:task.request.preset_id}}};
    j={id:++jobId,hostTaskId:task.id,submissionId:task.request.submission_id,input:{...input,src:await imageSource(input.resourceId)},card,...freeResultPosition(input)};jobs.push(j);changed=true;
   }
   if(j){
    const state=task.state==='succeeded'?'done':task.state;
    const failure=task.failure||null;
    if(j.state!==state||j.attempt!==task.attempt||JSON.stringify(j.failure||null)!==JSON.stringify(failure)){j.state=state;j.attempt=task.attempt;j.failure=failure;changed=true;}
    j.hostTaskId=task.id;
   }
   if(task.state==='succeeded')for(const output of task.outputs){
    const receipt=task.id+':'+task.attempt+':'+output.resource_id;if(placedTaskOutputs.has(receipt))continue;
    await persistDurable();
    const scene=await sceneClient.place(task,output,current=>{
     if(j)return {x:j.x,y:j.y};
     const input=current.items.find(i=>task.request.inputs.some(r=>r.resource_id===i.resourceId))||{x:0,y:0};
     let x=input.x+198,y=input.y;while(current.items.some(i=>Math.abs(i.x-x)<190&&Math.abs(i.y-y)<150))y+=160;
     return {x,y};
    });
    if(gesture||panelLayout?.active)return;
    nextId=Math.max(nextId,scene.nextId||nextId);
    for(const row of scene.items.filter(row=>row.taskId===task.id&&row.attempt===task.attempt&&row.resourceId===output.resource_id)){if(items.some(item=>item.id===row.id))continue;const data=await imageSource(row.resourceId);if(gesture||panelLayout?.active)return;items.push({...row,src:data});changed=true;}
    placedTaskOutputs.add(receipt);
   }
  }
  if(changed){await persistDurable();renderImages();renderStack();renderOutline();}else if(renderedImageWork!==JSON.stringify(taskMirror.imageWork||null)){renderJobs();}
 }catch(error){notice(String(error));}finally{taskSyncBusy=false;}
}
const jobs=[];const localUrls=new Set();let jobId=0;
const selection=new Set();let spaceHeld=false;
const marquee=document.createElement("div");marquee.className="selection-marquee";marquee.hidden=true;document.body.append(marquee);
const reduced=matchMedia('(prefers-reduced-motion: reduce)').matches;
let stackHovered=null,suppressStackClick=false;
let panelLayout,panelMotion,externalDragging=false,blurTimer;
const dropGate=quickgenBoardMotion.createGate();
let nativeTimer,nativeBusy=false,nativeDirty=false,managerOpening=false;
const cards=new Map();
function renderCards(abilities){cards.clear();$('abilities').replaceChildren();for(const ability of abilities){const card=document.createElement('div'),img=document.createElement('img'),label=document.createElement('span');card.className='ability';card.dataset.preset=ability.id;card.setAttribute('aria-label',ability.name+'：拖入图片执行');img.src=ability.cover||src(presets.some(p=>p[0]===ability.id)?ability.id:'clay');img.alt='';label.textContent=ability.name;card.append(img,label);card.tabIndex=0;
 const modelMenu=()=>{if(card.querySelector('.preset-model-menu'))return;const action=document.createElement('button');action.className='preset-model-menu';action.textContent='模型设置';action.onpointerdown=e=>{e.preventDefault();e.stopPropagation();};action.onclick=e=>{e.stopPropagation();action.remove();card.focus();manager('binding',ability.id);};action.onkeydown=e=>{if(e.key==='Escape'){e.stopPropagation();action.remove();card.focus();}};card.append(action);action.focus();};
 card.oncontextmenu=e=>{e.preventDefault();e.stopPropagation();modelMenu();};card.onkeydown=e=>{if(e.key==='ContextMenu'||(e.shiftKey&&e.key==='F10')){e.preventDefault();modelMenu();}};
 $('abilities').append(card);cards.set(ability.id,{el:card,name:ability.name,prompt:ability.prompt})}}
renderCards(initialConfig.abilities);
// Native focus may transition during mouse activation, before click. Dismiss
// explicitly outside the action instead of deleting its click target on blur.
document.addEventListener('pointerdown',event=>{
 if(event.target.closest('.preset-model-menu'))return;
 document.querySelectorAll('.preset-model-menu').forEach(menu=>menu.remove());
},true);

const hints=new quickgenHints.Hints(renderHint);
$('panel-hint').append($('receipt'));
function renderHint(row){
 const el=$('panel-hint');el.hidden=!row;
 $('receipt').hidden=row?.id!=='tasks';$('hint-text').hidden=row?.id==='tasks';
 $('hint-text').textContent=row?.text||'';
 $('hint-action').hidden=!row?.action;$('hint-action').textContent=row?.actionText||'';
 $('hint-action').onclick=()=>row?.action?.();
 $('hint-close').hidden=!row?.dismissible;
 $('hint-close').onclick=()=>hints.clear(row.id);
 syncNativeSoon();
}
function positionHint(){
 const el=$('panel-hint'),tools=$('ink-tools'),visibility=$('dock').hidden||panelMotion?.active?'hidden':'visible';el.style.visibility=visibility;tools.style.visibility=visibility;
 let panel=$('panel').getBoundingClientRect();el.style.width=Math.min(panel.width,innerWidth)+'px';tools.style.width=Math.min(Math.max(panel.width,252),innerWidth)+'px';const toolHeight=tools.hidden?0:tools.offsetHeight+6;
 panelLayout?.setBottomInset(toolHeight+(el.hidden?0:el.offsetHeight+6));panel=$('panel').getBoundingClientRect();
 tools.style.left=Math.max(0,Math.min(panel.left,innerWidth-tools.offsetWidth))+'px';tools.style.top=Math.max(0,Math.min(panel.bottom+6,innerHeight-tools.offsetHeight))+'px';
 el.style.left=Math.max(0,Math.min(panel.left,innerWidth-el.offsetWidth))+'px';el.style.top=Math.max(0,Math.min(panel.bottom+toolHeight+6,innerHeight-el.offsetHeight))+'px';
}
function offerUndo(note){
 const restore=()=>{
  if(quitting||noteBarrier)return;
  trackBoardWork((async()=>{try{if(notesMode.active){await switchNotes(false);if(notesMode.active)return;}await mutateNote(invoke('note_restore',{id:note.id}));hints.clear('undo:'+note.id);hints.clear('restore:'+note.id)}catch(error){hints.set('restore:'+note.id,{priority:40,text:'撤销未完成：'+error,actionText:'重试',action:restore});}})());
 };
 hints.set('undo:'+note.id,{priority:30,text:'便签已删除',actionText:'撤销',dismissible:true,action:restore});
}
function notice(text){clearTimeout(noticeTimer);$('notice').textContent=text;$('notice').hidden=false;noticeTimer=setTimeout(()=>$('notice').hidden=true,2400)}
function camera(){saveSoon(); $('world').style.transform=`translate(${tx}px,${ty}px) scale(${scale})`;$('zoom-value').textContent=Math.round(scale*100)+'%';$('zoom-out').disabled=scale<=.35;$('zoom-in').disabled=scale>=2.5 }
function zoom(next,x=innerWidth/2,y=innerHeight/2){next=Math.max(.35,Math.min(2.5,next));tx=x-(x-tx)*next/scale;ty=y-(y-ty)*next/scale;scale=next;camera()}
function arrange(){const r=$('dock').getBoundingClientRect(),cols=r.left>440?2:1;scale=innerWidth<700?.72:1;tx=0;ty=0;const w=180*scale,h=160*scale,gap=18*scale;const startX=Math.max(16,r.left>440?r.left-80-cols*w-(cols-1)*gap:r.right+72),startY=Math.max(78,r.top-24);items.forEach((item,i)=>{item.x=(startX+(i%cols)*(w+gap))/scale;item.y=(startY+Math.floor(i/cols)*h)/scale});placed=true;renderImages();camera()}
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
function renderImages(){ $('world').replaceChildren();items.forEach(item=>{const el=document.createElement('div');el.className='board-image'+(item.result?' simulated-result':'');el.tabIndex=0;el.title='按住 Ctrl 拖出到文件夹（复制图片）';el.dataset.image=item.id;el.setAttribute('aria-label',item.name+'，可拖动到能力卡片');el.style.left=item.x+'px';el.style.top=item.y+'px';const figure=document.createElement('figure'),img=document.createElement('img'),caption=document.createElement('figcaption'),remove=document.createElement('button');img.src=item.src;img.alt=item.name;img.draggable=false;caption.textContent=item.name;remove.className='remove';remove.setAttribute('aria-label','移除'+item.name);remove.innerHTML=icon('x');remove.onclick=e=>{e.stopPropagation();const index=items.indexOf(item);if(index>=0)items.splice(index,1);selection.delete(item.id);if(selected===item.id)selected=[...selection].at(-1)||null;renderImages();renderStack();renderOutline()};figure.append(img,remove);el.append(figure,caption);el.onpointerdown=e=>startDrag(e,item,el);el.onfocus=()=>{if(!selection.has(item.id))selectImage(item.id)};el.classList.toggle('selected',selection.has(item.id));el.onkeydown=e=>{if(['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(e.key)){e.preventDefault();item.x+=(e.key==='ArrowRight'?12:e.key==='ArrowLeft'?-12:0);item.y+=(e.key==='ArrowDown'?12:e.key==='ArrowUp'?-12:0);el.style.left=item.x+'px';el.style.top=item.y+'px'}};$('world').append(el)});renderJobs();paintDesktopPeek() }
function centerOutline(animate=false){
 if(!open||!selected)return;const rail=$('outline'),thumb=[...rail.querySelectorAll('.outline-thumb')].find(n=>n.dataset.image===selected);if(!thumb)return;
 rail.style.setProperty('--edge-pad',Math.max(0,(rail.clientHeight-34)/2)+'px');
 const r=rail.getBoundingClientRect(),t=thumb.getBoundingClientRect();rail.scrollTo({top:rail.scrollTop+t.top+t.height/2-r.top-r.height/2,behavior:animate&&!reduced?'smooth':'instant'});
}
function paintSelection(){document.querySelectorAll('.board-image,.outline-thumb').forEach(n=>{const active=selection.has(n.dataset.image);n.classList.toggle('selected',active);if(n.classList.contains('outline-thumb'))n.setAttribute('aria-pressed',String(active))})}
function selectImage(id,center=true){selected=id;selection.clear();if(id)selection.add(id);paintSelection();if(center)centerOutline(true)}
function renderOutline(){const list=$('outline-list');list.replaceChildren();[...items].sort((a,b)=>b.addedAt-a.addedAt||b.order-a.order).forEach(item=>{const thumb=document.createElement('button');thumb.className='outline-thumb';thumb.dataset.image=item.id;thumb.dataset.addedAt=String(item.addedAt);thumb.setAttribute('aria-label','大纲：'+item.name);thumb.setAttribute('aria-pressed',String(selected===item.id));const img=document.createElement('img');img.src=item.src;img.alt='';img.draggable=false;thumb.append(img);thumb.onpointerdown=e=>startDrag(e,item,thumb,'outline');thumb.onclick=()=>selectImage(item.id);list.append(thumb)});if(!selected){$('outline').style.setProperty('--edge-pad','0px');$('outline').scrollTop=0}paintSelection();centerOutline()}
new ResizeObserver(()=>{if(open&&!gesture)centerOutline()}).observe($('panel'));
function showCanvas(instant=false){if(!items.length){$('files').click();return}open=true;$('canvas').hidden=false;$('canvas-tools').hidden=false;$('outline').hidden=false;$('stack').setAttribute('aria-expanded','true');if(!placed)arrange();renderOutline();if(!reduced&&!instant)$('canvas').animate([{opacity:0},{opacity:1}],{duration:180,easing:'ease-out'});camera()}
function hideCanvas(){setPassThrough(false);cancelGesture();open=false;$('canvas').hidden=true;$('canvas-tools').hidden=true;$('outline').hidden=true;$('stack').setAttribute('aria-expanded','false');saveSoon()}
function setPassThrough(value,instant=false){
 panelLayout?.cancel();
 clearDesktopPeek();
 passthrough=Boolean(value&&open);cancelGesture();spaceHeld=false;$('canvas').classList.remove('pan-ready');document.body.classList.toggle('desktop-mode',passthrough);$('canvas').inert=passthrough;$('outline').inert=passthrough;
 
 $('canvas-mode').setAttribute('aria-pressed',String(passthrough));$('canvas-mode').setAttribute('aria-label',passthrough?'恢复画板操作模式':'切换桌面穿透模式');$('canvas-mode').innerHTML=icon(passthrough?'panel-top':'monitor');
 $('outline').hidden=passthrough||!open;setPanelVisible(!passthrough||pinned,instant);
 saveSoon();
}
$('canvas-mode').onclick=()=>setPassThrough(!passthrough);
function paintDesktopPeek(){for(const el of document.querySelectorAll('#world .board-image')){const hit=passthrough&&open&&desktopPeekIds.has(el.dataset.image);if(el.classList.contains('desktop-peek')!==hit)el.classList.toggle('desktop-peek',hit)}}
function clearDesktopPeek(){desktopPeekIds.clear();paintDesktopPeek()}
function setPanelVisible(visible,instant=false){if(!panelMotion){$('dock').hidden=!visible;$('notch').hidden=visible;return Promise.resolve(true)}if(visible===panelMotion.visible&&!panelMotion.active)return Promise.resolve(true);if(visible)dropGate.opening();else dropGate.closed();return panelMotion.set(visible,instant).then(done=>{if(done&&visible)dropGate.expanded();return done})}
function restorePanel(instant=false){return setPanelVisible(true,instant)}
function collapse(instant=false){hideCanvas();setPanelVisible(false,instant);syncNativeSoon()}
function targetAt(x,y){return [...cards.values()].find(card=>{const r=card.el.getBoundingClientRect();return x>=r.left&&x<=r.right&&y>=r.top&&y<=r.bottom})}
function highlight(card){cards.forEach(c=>c.el.classList.toggle('target',c===card))}
function freeResultPosition(input){
 const occupied=[...items,...jobs.filter(j=>j.state!=='done')];
 for(let ring=1;ring<60;ring++)for(const [dx,dy] of [[-198*ring,0],[198*ring,0],[0,160*ring],[0,-160*ring]]){
 const x=input.x+dx,y=input.y+dy;const panel=$('dock').getBoundingClientRect();if(x*scale+tx+180*scale>panel.left-64&&x*scale+tx<panel.right&&y*scale+ty+144*scale>panel.top-80&&y*scale+ty<panel.bottom+40)continue;
 if(!occupied.some(i=>Math.abs(i.x-x)<190&&Math.abs(i.y-y)<150))return {x,y};
 }return {x:input.x-198,y:input.y};
}
function jobIsMock(j){return taskMirror.tasks.get(j.hostTaskId)?.request.capability.connection_id==='internal-mock';}
function jobSaveFailed(j){
 const work=taskMirror.imageWork,task=taskMirror.tasks.get(j.hostTaskId);
 return !!(work?.save_failed&&task&&work.task_id===task.id&&work.attempt===task.attempt&&work.event_version===task.event_version);
}
function jobStatus(j){
 if(jobSaveFailed(j)||j.failure?.stage==='archive')return '结果保存失败';
 if(j.admissionError)return '提交失败';
 return ({failed:jobIsMock(j)?'模拟失败':'生成失败',interrupted:'已中断',cancelled:'已取消',running:jobIsMock(j)?'模拟生成中…':'生成中…',finalizing:'保存结果中…',queued:'等待生成'})[j.state];
}
function renderJobs(){saveSoon();renderedImageWork=JSON.stringify(taskMirror.imageWork||null);
 document.querySelectorAll('.generation-job').forEach(n=>n.remove());
 for(const j of jobs.filter(j=>j.state!=='done'&&!j.hidden)){
 const el=document.createElement('div');el.className='generation-job '+j.state+(j.failure||jobSaveFailed(j)?' failed':'');el.dataset.job=j.id;el.style.left=j.x+'px';el.style.top=j.y+'px';el.setAttribute('role','status');
 const name=document.createElement('strong');name.textContent=j.input.name+' · '+j.card.name;
 const status=document.createElement('span');const actions=document.createElement('div');actions.className='job-actions';status.textContent=jobStatus(j);el.append(name,status);
 if(j.failure){const error=document.createElement('span');error.className='job-error';error.textContent=j.failure.message;error.title=j.failure.message;el.append(error)}
 if(j.state==='finalizing'&&j.failure?.stage==='archive'){const retry=document.createElement('button');retry.textContent='重试保存';retry.setAttribute('aria-label','重试保存 '+j.input.name);retry.onclick=()=>durableAction(j,'retry_archive');actions.append(retry)}
 if(['failed','interrupted','cancelled'].includes(j.state)){const retry=document.createElement('button');retry.textContent='重试';retry.setAttribute('aria-label','重试 '+j.input.name);retry.onclick=()=>durableAction(j,'retry');actions.append(retry)}
 const action=document.createElement('button');const cancellable=j.state==='queued';action.textContent=cancellable?'取消':'隐藏';action.title=cancellable?'取消尚未执行的任务':'仅隐藏任务卡片；不取消执行，不删除结果';action.setAttribute('aria-label',action.textContent+'任务 '+j.input.name);action.onclick=()=>{if(cancellable)durableAction(j,'cancel');else{j.hidden=true;saveScene();renderJobs()}};actions.append(action);el.append(actions);
 el.onpointerdown=e=>e.stopPropagation();$('world').append(el);
 }
 const pending=jobs.filter(j=>(j.state==='running'||j.state==='queued')&&!jobSaveFailed(j)).length,failed=jobs.filter(j=>j.state==='failed'||j.failure?.stage==='archive'||jobSaveFailed(j)).length;
 $('receipt-show').hidden=!jobs.some(j=>j.hidden&&j.state!=='done');
 if(!jobs.length)hints.clear('tasks');
 if(jobs.length){$('receipt').hidden=false;$('receipt-name').textContent=pending?pending+' 张处理中':failed?failed+' 张失败':jobs.some(j=>j.state!=='done')?'有未完成任务':'全部完成';$('receipt').querySelector('span').textContent=(jobs.every(jobIsMock)?'逐张模拟':'逐张处理')+' · '+jobs.filter(j=>j.state==='done').length+' 张完成';$('receipt-image').src=jobs.at(-1).input.src;hints.set('tasks',{priority:10});}
}
function simulate(inputs,card){
 if(!storageReady){notice('画板尚未恢复，无法提交任务');return;}
 const failFirst=$('simulate-failure').checked;$('simulate-failure').checked=false;
 const added=[],batchId=crypto.randomUUID();inputs.forEach((input,index)=>{const position=freeResultPosition(input);const job={id:++jobId,input:{...input},card,state:'queued',fail:failFirst&&index===0,...position};job.submissionId=crypto.randomUUID();job.batchId=batchId;added.push(job);jobs.push(job)});
 renderJobs();
 trackBoardWork((async()=>{for(const job of added){try{await submitDurable(job)}catch(error){notice(String(error));renderJobs()}}await syncDurableTasks();})());
}
async function dragOut(item){
 if(outboundDragging||notesMode.active||passthrough||quitting||!item.resourceId)return;
 outboundDragging=true;cancelGesture();
 try{await invoke('resource_drag_out',{id:item.resourceId});hints.clear('drag-output');}
 catch(error){hints.set('drag-output',{priority:20,text:'拖出未完成：'+String(error)});}
 finally{outboundDragging=false;syncNativeSoon();}
}
function startDrag(e,item,el,source='canvas'){
 if(e.button===0&&e.ctrlKey&&!e.altKey&&!e.shiftKey&&source==='canvas'&&!e.target.closest('button')){e.preventDefault();e.stopPropagation();trackBoardWork(dragOut(item));return;}

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
$('stack').onclick=()=>{if(inkTools?.active){trackBoardWork(switchInk(false).then(()=>{if(!inkTools.active)showCanvas()})).catch(()=>{});return;}if(notesMode.active)return;if(suppressStackClick){suppressStackClick=false;return}open?hideCanvas():showCanvas()};$('canvas-close').onclick=hideCanvas;$('zoom-in').onclick=()=>zoom(scale*1.2);$('zoom-out').onclick=()=>zoom(scale/1.2);$('zoom-value').onclick=()=>zoom(1);$('fit').onclick=arrange;
$('pin').onclick=()=>{pinned=!pinned;$('pin').setAttribute('aria-pressed',String(pinned));$('pin').setAttribute('aria-label',pinned?'取消常驻面板':'常驻面板');if(passthrough)setPassThrough(true)};$('notch').onclick=()=>{restorePanel();if(passthrough)setPassThrough(false)};
$('theme').onclick=async()=>{const next=document.body.classList.contains('light')?'dark':'light';try{await invoke('save_config',{section:'theme',value:next});applyTheme(next)}catch(e){notice(String(e))}};
$('desktop').onpointerdown=()=>{if(!pinned&&!open)collapse()};
document.addEventListener('keydown',e=>{if(e.key==='Escape'){if(gesture){cancelGesture();return}if(open){hideCanvas();$('stack').focus()}else if(!pinned)collapse(true)}});
const noteMutations=new Set();
function mutateNote(work){noteMutations.add(work);work.finally(()=>noteMutations.delete(work)).catch(()=>{});return trackBoardWork(work);}
let overviewRefresh=0;
const overviewView=quickgenNotesOverview.mount({layer:$('notes-overview'),onError:error=>notice(String(error)),onToggle:note=>{
 if(quitting||noteBarrier)return Promise.resolve();
 return mutateNote((async()=>{await invoke('note_set_resident',{id:note.id,resident:!note.resident});await refreshNotesOverview();})());
}});
const notesMode=quickgenNotesOverview.createMode({
 list:()=>invoke('notes_list'),flush:()=>invoke('notes_flush'),present:overview=>invoke('notes_present',{overview}),release:()=>invoke('notes_release'),
 render:notes=>{overviewView.reset();$('notes-overview').hidden=false;overviewView.render(notes);},
 changed:active=>{document.body.classList.toggle('notes-overview-mode',active);$('notes-overview').hidden=!active;$('notes-entry').setAttribute('aria-expanded',String(active));if(!active)overviewView.cancel();syncNativeSoon();},
 empty:()=>hints.set('notes-empty',{priority:20,text:'右键便签按钮，新建便签',dismissible:true})
});
async function refreshNotesOverview(){const sequence=++overviewRefresh;if(!notesMode.active)return;const notes=await invoke('notes_list');if(sequence===overviewRefresh&&notesMode.active)overviewView.render(notes.filter(n=>!n.deleted));}
async function switchNotes(value){
 if(noteBarrier||quitting)return;
 if(value&&inkTools?.active){await switchInk(false);if(inkTools.active)return;}
 noteBarrier=true;cancelGesture();
 try{await Promise.allSettled([...noteMutations]);await notesMode.set(value);hints.clear('notes-mode-error');}
 catch(error){hints.set('notes-mode-error',{priority:40,text:'便签切换未完成：'+error,actionText:'重试',action:()=>trackBoardWork(switchNotes(value))});}
 finally{noteBarrier=false;}
}
async function switchInk(value){
 if(quitting||noteBarrier)return;
 try{if(value&&notesMode.active){await switchNotes(false);if(notesMode.active)return;}if(value){await syncNative();}await inkTools.set(value);hints.clear('ink-mode-error');}
 catch(error){hints.set('ink-mode-error',{priority:40,text:'画笔切换未完成：'+error,actionText:'重新打开',action:()=>trackBoardWork(switchInk(true))});throw error;}
}
async function clearInk(){if(quitting||noteBarrier)return;try{await inkTools.clear();hints.clear('ink-clear-error');}catch(error){hints.set('ink-clear-error',{priority:40,text:'画笔清空未确认：'+error,actionText:'重试清空',action:()=>trackBoardWork(clearInk())});}}
inkTools=quickgenInkTools.mount({onClear:()=>trackBoardWork(clearInk()),entry:$('ink-entry'),row:$('ink-tools'),invoke,onToggle:value=>trackBoardWork(switchInk(value)).catch(()=>{}),onChange:syncNativeSoon,onSaved:()=>hints.clear('ink-style-error'),onError:error=>hints.set('ink-style-error',{priority:40,text:'画笔设置未保存：'+error,dismissible:true})});
const inkChannel=new window.__TAURI__.core.Channel();inkChannel.onmessage=data=>inkTools.consume(data);await invoke('ink_subscribe',{channel:inkChannel});
await window.__TAURI__.event.listen('ink-error',e=>{const key='ink-error:'+e.payload.id;if(e.payload.error)hints.set(key,{priority:40,text:'画笔更改未确认：'+e.payload.error,actionText:'打开画笔',action:()=>trackBoardWork(switchInk(true)).catch(()=>{})});else hints.clear(key);},{target:{kind:'WebviewWindow',label:'main'}});
await inkTools.load();
document.addEventListener('keydown',e=>{if(e.key==='Escape'&&inkTools.active){e.preventDefault();e.stopImmediatePropagation();trackBoardWork(switchInk(false)).catch(()=>{});}},true);
$('notes-entry').oncontextmenu=e=>{e.preventDefault();if(quitting||noteBarrier)return;trackBoardWork((async()=>{try{if(notesMode.active){await switchNotes(false);if(notesMode.active)return;}await mutateNote((async()=>{const note=await invoke('note_create');hints.clear('notes-empty');await invoke('note_open',{id:note.id});})())}catch(error){notice('便签创建或打开失败：'+error)}})());};
$('notes-entry').onclick=()=>{if(quitting||noteBarrier)return;trackBoardWork(switchNotes(!notesMode.active));};
document.addEventListener('keydown',e=>{if(!notesMode.active)return;if(e.key==='Escape'){e.preventDefault();e.stopImmediatePropagation();trackBoardWork(switchNotes(false));}else if(e.code==='Space')e.stopImmediatePropagation();},true);
$('receipt-show').onclick=()=>trackBoardWork((async()=>{if(inkTools?.active)await switchInk(false);if(notesMode.active){await switchNotes(false);if(notesMode.active)return;}for(const j of jobs)j.hidden=false;showCanvas(true);renderJobs();})());
$('receipt-close').onclick=()=>hints.clear('tasks');
let copyingImage=false;
async function copyImage(item){
 if(copyingImage)return;copyingImage=true;
 try{const result=await invoke('resource_copy_image',{id:item.resourceId});if(!result?.copied)throw Error('未收到复制确认');hints.clear('clipboard-output');}
 catch(error){hints.set('clipboard-output',{priority:20,text:'复制图片失败：'+String(error)});}
 finally{copyingImage=false;}
}
document.addEventListener('copy',event=>{
 if(event.defaultPrevented||!document.hasFocus()||!open||passthrough||notesMode.active||noteBarrier||quitting||managerOpening||!storageReady||outboundDragging)return;
 if((event.composedPath?.()||[event.target]).some(el=>el?.isContentEditable||el?.matches?.('input,textarea,select,[role="textbox"],[role="dialog"]'))||window.getSelection()?.isCollapsed===false)return;
 if(selection.size!==1)return;const item=items.find(i=>selection.has(i.id));if(!item?.resourceId)return;
 event.preventDefault();trackBoardWork(copyImage(item));
});
document.addEventListener('paste',quickgenClipboardIntake.create({allowed:()=>document.hasFocus()&&!passthrough&&!notesMode.active&&!noteBarrier&&!quitting&&!managerOpening&&storageReady,importFiles,clear:()=>hints.clear('clipboard-input'),report:text=>hints.set('clipboard-input',{priority:20,text})}));
let picking=false;$('import').onclick=()=>{picking=true;$('files').click()};$('files').onchange=async e=>{picking=false;await importFiles([...e.target.files]);e.target.value=''};$('files').addEventListener('cancel',()=>picking=false);
function importFiles(files){if(inkTools?.active)return trackBoardWork(switchInk(false).then(()=>importFiles(files)));if(notesMode.active)return trackBoardWork((async()=>{await switchNotes(false);if(!notesMode.active)return await importFiles(files);})());if(!storageReady){notice('画板尚未恢复，无法导入');return Promise.resolve({added:0});}return trackBoardWork(importFilesWork(files));}
async function importFilesWork(files){let added=0;for(const file of files){if(!/^image\/(png|jpeg|webp)$/.test(file.type)||file.size>20*1024*1024){notice('请选择20 MB以内的PNG、JPG或WebP');continue}const url=await new Promise((resolve,reject)=>{const reader=new FileReader();reader.onload=()=>resolve(reader.result);reader.onerror=reject;reader.readAsDataURL(file)}),img=new Image();img.src=url;try{await img.decode()}catch{URL.revokeObjectURL(url);notice('图片无法读取');continue}let record;try{record=await invoke('resource_import',{data:url,name:file.name});resourceImages.set(record.resource.id,url)}catch(error){notice(String(error));continue}const r=$('dock').getBoundingClientRect(),i=items.length;items.push({id:'local-'+nextId++,name:file.name,src:url,resourceId:record.resource.id,local:true,addedAt:Date.now(),order:nextId,x:(Math.max(16,r.left>440?r.left-410:r.right+72)+(i%2)*198-tx)/scale,y:(Math.max(80,r.top-24)+Math.floor(i/2)*160-ty)/scale});added++}if(added){restorePanel();renderStack();renderImages();renderOutline();if(!open)showCanvas()}return {added};}
document.addEventListener('dragover',e=>{if([...e.dataTransfer.types].includes('Files')){e.preventDefault();document.body.classList.add('file-over')}});document.addEventListener('dragleave',e=>{if(!e.relatedTarget)document.body.classList.remove('file-over')});document.addEventListener('drop',async e=>{if(!e.dataTransfer.files.length)return;e.preventDefault();document.body.classList.remove('file-over');await importFiles([...e.dataTransfer.files])});
window.addEventListener('resize',()=>{if(open&&placed){camera()}});window.addEventListener('beforeunload',()=>localUrls.forEach(url=>URL.revokeObjectURL(url)));
async function initStorage(){
 document.body.inert=true;
 try{
 const status=await invoke('storage_status');let state=status.scene;
 if(!state){
  if(!status.activationAllowed)throw Error('本候选尚未开放生产迁移，请保留原现场并使用已验收版本');
  const db=await new Promise((resolve,reject)=>{const r=indexedDB.open('quickgen-board-single-v1',1);r.onupgradeneeded=()=>r.result.createObjectStore('scene');r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(r.error)});
  try{state=await new Promise((resolve,reject)=>{const r=db.transaction('scene').objectStore('scene').get('current');r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(r.error)});}finally{db.close();}
  state=await invoke('migration_activate',{snapshot:state||snapshotScene()});
 }
 if(state?.version!==1||state.resourceSchemaVersion!==1)throw Error('资源场景版本不受支持，请保留本地数据');
 durable=!!state?.resourceSchemaVersion;
 if(durable){sceneClient.current=structuredClone(state);lastLocalScene=structuredClone(state);state=structuredClone(state);for(const item of state.items)item.src=await imageSource(item.resourceId);for(const job of state.jobs)job.input.src=await imageSource(job.input.resourceId);}
 if(state?.version===1){items.splice(0,items.length,...state.items);jobs.splice(0,jobs.length,...state.jobs.map(j=>({...j,card:{name:j.abilitySnapshot?.name||'已移除能力',prompt:j.abilitySnapshot?.prompt||'',el:{dataset:{preset:j.preset}}}})).filter(j=>j.card));scale=state.scale;tx=state.tx;ty=state.ty;placed=state.placed;nextId=state.nextId;jobId=state.jobId;pinned=state.pinned;selection.clear();state.selection.forEach(id=>{if(items.some(i=>i.id===id))selection.add(id)});selected=state.selected;document.body.classList.toggle('light',state.light);$('pin').setAttribute('aria-pressed',String(pinned));if(state.open&&items.length){showCanvas();setPassThrough(state.passthrough,true)}}
 storageReady=true;
 if(durable){await taskMirror.sync();for(const job of jobs)if(job.submissionId&&!job.hostTaskId){try{await submitDurable(job)}catch(error){notice(String(error))}}setInterval(syncDurableTasks,500);}
 }catch(error){storageError=String(error);notice('无法恢复本地画板，请保留当前页面：'+storageError);console.error(error)}
 finally{document.body.inert=false;renderStack();renderImages();renderOutline();camera()}
}
function saveSoon(){syncNativeSoon();if(!storageReady)return;clearTimeout(saveTimer);saveTimer=setTimeout(saveScene,150)}
function snapshotScene(){return structuredClone({version:1,items:items.map(i=>({...i})),jobs:jobs.map(({card,...j})=>({...j,preset:card.el.dataset.preset,abilitySnapshot:{name:card.name,prompt:card.prompt}})),scale,tx,ty,placed,nextId,jobId,pinned,selected,selection:[...selection],open,passthrough,light:document.body.classList.contains('light')})}
function saveScene(){if(!storageReady)return;clearTimeout(saveTimer);return persistDurable().catch(error=>{notice('保存失败：'+String(error));throw error});}
 document.addEventListener('pointerup',saveSoon);document.addEventListener('keyup',saveSoon);document.addEventListener('click',saveSoon);document.addEventListener('visibilitychange',()=>{if(document.hidden)saveScene()});window.addEventListener('pagehide',saveScene);
function applyTheme(mode){document.documentElement.dataset.theme=mode;document.body.classList.toggle('light',mode==='light'||(mode==='system'&&matchMedia('(prefers-color-scheme:light)').matches));$('theme').innerHTML=icon(document.body.classList.contains('light')?'sun':'moon')}
function rect(el,pad=0){const r=el.getBoundingClientRect();return{x:r.left-pad,y:r.top-pad,width:r.width+pad*2,height:r.height+pad*2}}
function syncNativeSoon(){if(nativeTimer)return;nativeTimer=setTimeout(()=>{nativeTimer=null;syncNative()},16)}
async function syncNative(){if(nativeBusy){nativeDirty=true;return}nativeBusy=true;try{positionHint();const rects=[],inkControls=[];for(const id of ['notes-overview','notes-entry','ink-entry','ink-tools','panel-hint','panel','stack','outline','notch','canvas-tools']){const el=$(id);if(!el.closest('[hidden]')&&getComputedStyle(el).display!=='none'&&getComputedStyle(el).visibility!=='hidden'){const bounds=rect(el,id==='stack'?32:6);rects.push(bounds);if(['ink-entry','ink-tools','panel-hint','notch'].includes(id))inkControls.push(bounds);}}const visuals=open&&passthrough?[...document.querySelectorAll('#world .board-image,#world .generation-job')].map(el=>rect(el,16)).filter(r=>r.x<innerWidth&&r.y<innerHeight&&r.x+r.width>0&&r.y+r.height>0):[];const hoverTargets=open&&passthrough?[...document.querySelectorAll('#world .board-image')].map(el=>({id:el.dataset.image,rect:rect(el)})):[];await invoke('board_view',{view:{open:open||notesMode.active,passthrough:notesMode.active?false:passthrough,pinned,visuals:notesMode.active?[]:visuals,hoverTargets:notesMode.active?[]:hoverTargets,interacting:Boolean(panelLayout?.active||gesture||panelMotion?.active),rects,inkControls,panel:rect($('panel'))}})}catch(e){notice(String(e))}finally{nativeBusy=false;if(nativeDirty){nativeDirty=false;syncNativeSoon()}}}
async function manager(mode,presetId){managerOpening=true;try{await syncNative();await invoke('open_manager',{mode,presetId});await invoke('show_manager')}catch(e){notice(String(e))}finally{managerOpening=false}}
$('add-ability').onclick=()=>manager('add');$('library').onclick=()=>manager('library');$('settings').onclick=()=>manager('settings');
await window.__TAURI__.event.listen('note-changed',e=>{const note=e.payload;if(note.deleted)offerUndo(note);else hints.clear('undo:'+note.id);if(notesMode.active)refreshNotesOverview().catch(error=>notice(String(error)));});
await window.__TAURI__.event.listen('note-save-error',e=>{const {id,error}=e.payload;const key='note-error:'+id;if(error)hints.set(key,{priority:40,text:'便签保存未完成：'+error,actionText:'查看',action:()=>{if(noteBarrier||quitting)return;trackBoardWork((async()=>{if(notesMode.active){await switchNotes(false);if(notesMode.active)return;}await mutateNote(invoke('note_open',{id}));})().catch(error=>notice(String(error))));}});else hints.clear(key);});
await window.__TAURI__.event.listen('config-changed',async e=>{if(e.payload==='layout')return;try{const cfg=await invoke('load_config');renderCards(cfg.abilities);applyTheme(cfg.theme);syncNativeSoon()}catch(e){notice(String(e))}});
function groupBlur(){clearTimeout(blurTimer);blurTimer=setTimeout(async()=>{if(outboundDragging||notesMode.active||inkTools?.active||noteBarrier||pinned||picking||managerOpening||panelLayout?.active||externalDragging)return;try{if(await invoke('board_group_focused'))return}catch(e){notice(String(e));return}if(open)setPassThrough(true);else collapse()},150)}
const nativeChannel=new window.__TAURI__.core.Channel();
nativeChannel.onmessage=async event=>{
 if(event.type==='recovery-export'){
  const reply=async(result,error)=>{if(event.requestId)await invoke('board_recovery_complete',{requestId:event.requestId,result:result||null,error:error?String(error):null});};
  if(quitting||noteBarrier){await reply(null,'正在保存或退出，请稍后重试');return;}
  if(!durable||!storageReady){await reply(null,'画板尚未切换到资源库，请先保留原现场');notice('画板尚未切换到资源库，请先保留原现场');return;}
  noteBarrier=true;const priorWork=[...pendingBoardWork];
  trackBoardWork((async()=>{try{
   await Promise.allSettled(priorWork);await invoke('notes_flush');await persistDurable();const result=await invoke('recovery_export');
   await reply(result,null);if(!event.requestId){notice('恢复副本已验证并保存');await invoke('board_recovery_open',{directory:result.directory});}
  }catch(error){await reply(null,error).catch(report=>notice(String(report)));notice('恢复副本未完成：'+String(error));}
  finally{if(!notesMode.active)await invoke('notes_release').catch(error=>notice(String(error)));noteBarrier=false;}})());return;
 }
 if(event.type==='flush-and-quit'){
  const exitTrace=async step=>{if(diagnostic==='generation-recovery')await invoke('board_capacity_progress',{result:{step,pending:pendingBoardWork.size,syncBusy:taskSyncBusy}});};
  await exitTrace('quit-received');
  if(quitting)return;quitting=true;document.body.inert=true;clearTimeout(saveTimer);
  try{
   await exitTrace('quit-wait-work');
   while(pendingBoardWork.size)await Promise.allSettled([...pendingBoardWork]);
   await exitTrace('quit-wait-sync');
   while(taskSyncBusy)await new Promise(resolve=>setTimeout(resolve,25));
   clearTimeout(saveTimer);
   await exitTrace('quit-save-scene');
   if(storageReady)await persistDurable();
   await invoke('ink_set_active',{enabled:false});await exitTrace('quit-flush-notes');await invoke('notes_flush');await exitTrace('quit-host-barrier');await invoke('board_quit_saved');
  }catch(error){if(!notesMode.active)await invoke('notes_release').catch(()=>{});quitting=false;document.body.inert=false;notice('退出前保存失败，请保留当前页面：'+String(error));if(diagnostic==='generation-recovery')await invoke('board_capacity_progress',{result:{step:'quit-error',error:String(error)}});}
  return;
 }
 if(event.type==='show-panel'){if(notesMode.active){await trackBoardWork(switchNotes(false));if(notesMode.active)return;}setPassThrough(false,true);await restorePanel(true);await syncNative()}
 if(event.type==='toggle'){if(inkTools?.active){await trackBoardWork(switchInk(false));return;}if(notesMode.active){await trackBoardWork(switchNotes(false));return;}if(open)setPassThrough(!passthrough,true);else{restorePanel(true);if(items.length)showCanvas(true)}syncNativeSoon()}
 if(event.type==='hover'&&passthrough&&open){desktopPeekIds=new Set(event.ids);paintDesktopPeek()}
 if(event.type==='group-blur')groupBlur();
 if(event.type==='display'){panelMotion?.settle();cancelGesture();trackBoardWork(invoke('notes_rehome').then(()=>hints.clear('note-display-error')).catch(error=>hints.set('note-display-error',{priority:40,text:'便签窗口恢复未完成：'+error,actionText:'重试',action:()=>trackBoardWork(invoke('notes_rehome').then(()=>hints.clear('note-display-error')).catch(error=>notice(String(error))))})));syncNativeSoon()}
 if(event.type==='error')notice(event.message);
 if(event.type==='enter'){externalDragging=true;restorePanel();document.body.classList.add('file-over');syncNativeSoon()}
 if(event.type==='over'){const factor=window.devicePixelRatio||1;highlight(dropGate.allows()&&!panelMotion?.active?targetAt(event.position.x/factor,event.position.y/factor):null)}
 if(event.type==='leave'){externalDragging=false;document.body.classList.remove('file-over');highlight(null);groupBlur()}
 if(event.type==='drop'){
  externalDragging=false;document.body.classList.remove('file-over');const factor=window.devicePixelRatio||1;const card=dropGate.allows()&&!panelMotion?.active?targetAt(event.position.x/factor,event.position.y/factor):null;
  const files=[];for(const path of event.paths){try{const image=await invoke('read_image',{path});const [head,encoded]=image.data.split(',');const bytes=Uint8Array.from(atob(encoded),c=>c.charCodeAt(0));files.push(new File([bytes],image.name,{type:head.slice(5,head.indexOf(';'))}))}catch(e){notice(String(e))}}
  const before=new Set(items.map(i=>i.id));await importFiles(files);const added=items.filter(i=>!before.has(i.id));if(card&&added.length)simulate(added,card);highlight(null);syncNativeSoon();
 }
};
await invoke('board_subscribe',{channel:nativeChannel});
new ResizeObserver(syncNativeSoon).observe($('dock'));
window.addEventListener('resize',syncNativeSoon);
panelLayout=quickgenPanelLayout.mount({initial:initialConfig.layout,onChange:()=>{panelMotion?.settle();if(panelLayout?.active)syncNative();else syncNativeSoon();if(open)centerOutline()},onSave:value=>invoke('save_config',{section:'layout',value}),onError:e=>notice(String(e))});
panelMotion=quickgenBoardMotion.mount({dock:$('dock'),notch:$('notch'),prepare:syncNative,changed:syncNativeSoon});
const boardCapacity=diagnostic==='reliable-capacity'&&(await invoke('storage_status')).isolated;
const boardRehearsal=diagnostic==='reliable-rehearsal'&&(await invoke('storage_status')).isolated;
if(boardRehearsal){await new Promise((resolve,reject)=>{const script=document.createElement('script');script.src='board-rehearsal.js';script.onload=resolve;script.onerror=reject;document.head.append(script)});await quickgenBoardRehearsal.seed();}
const generationRecovery=diagnostic==='generation-recovery'&&(await invoke('storage_status')).isolated;
const boardReview=diagnostic==='reliable-review'&&(await invoke('storage_status')).isolated;
const boardSmoke=diagnostic==='reliable-base'&&(await invoke('storage_status')).isolated;
if(boardCapacity){await new Promise((resolve,reject)=>{const script=document.createElement('script');script.src='board-capacity.js';script.onload=resolve;script.onerror=reject;document.head.append(script)});await quickgenBoardCapacity.seed();}
if(boardReview||generationRecovery){await new Promise((resolve,reject)=>{const script=document.createElement('script');script.src='board-smoke.js';script.onload=resolve;script.onerror=reject;document.head.append(script)});await quickgenBoardSmoke.seed();await new Promise((resolve,reject)=>{const script=document.createElement('script');script.src='board-review.js';script.onload=resolve;script.onerror=reject;document.head.append(script)});}
if(boardSmoke){await new Promise((resolve,reject)=>{const script=document.createElement('script');script.src='board-smoke.js';script.onload=resolve;script.onerror=reject;document.head.append(script)});await quickgenBoardSmoke.seed();}
if(generationRecovery)await invoke('board_capacity_progress',{result:{step:'before-init-storage'}});
await initStorage();if(generationRecovery)await invoke('board_capacity_progress',{result:{step:'before-restore-notes'}});try{const restoredNotes=await invoke('notes_list');for(const note of restoredNotes.filter(n=>n.deleted))offerUndo(note);for(const note of restoredNotes.filter(n=>n.resident&&!n.deleted))await invoke('note_open',{id:note.id});}catch(error){notice('便签恢复未完成，可重试打开：'+error)}applyTheme(initialConfig.theme);if(diagnostic&&!boardSmoke&&!boardCapacity&&!boardRehearsal&&!boardReview&&items.length){showCanvas(true);setPassThrough(true,true)}syncNativeSoon();
if(generationRecovery){await invoke('board_capacity_progress',{result:{step:'board-ready'}});await new Promise((resolve,reject)=>{const script=document.createElement('script');script.src='generation-recovery.js';script.onload=resolve;script.onerror=reject;document.head.append(script)});quickgenGenerationRecovery.run({items,jobs,cards,simulate,sync:syncDurableTasks,save:()=>persistDurable(),busy:()=>taskSyncBusy,work:()=>taskMirror.imageWork});}
if(boardReview)quickgenBoardReview.run({items,jobs,cards,simulate,sync:syncDurableTasks,save:()=>persistDurable(),busy:()=>taskSyncBusy});
if(boardSmoke)quickgenBoardSmoke.run({items,jobs,cards,selection,durable:()=>durable,importFiles,simulate,sync:syncDurableTasks,save:()=>persistDurable(),select:selectImage,hide:hideCanvas,show:showCanvas,busy:()=>taskSyncBusy});
if(boardCapacity)quickgenBoardCapacity.run({items,jobs,cards,simulate,sync:syncDurableTasks,save:()=>persistDurable(),hide:hideCanvas,show:showCanvas,pass:setPassThrough});
if(diagnostic==='notes-restart'&&(await invoke('storage_status')).isolated){await new Promise((resolve,reject)=>{const script=document.createElement('script');script.src='notes-restart.js';script.onload=resolve;script.onerror=reject;document.head.append(script)});quickgenNotesRestart.run({set:async value=>{await switchNotes(value);if(notesMode.active!==value)throw Error('notes mode transition failed')}});}
if(diagnostic==='notes-fault'&&(await invoke('storage_status')).isolated){await new Promise((resolve,reject)=>{const script=document.createElement('script');script.src='notes-fault.js';script.onload=resolve;script.onerror=reject;document.head.append(script)});quickgenNotesFault.run();}
if(diagnostic==='notes-capacity'&&(await invoke('storage_status')).isolated){await new Promise((resolve,reject)=>{const script=document.createElement('script');script.src='notes-capacity.js';script.onload=resolve;script.onerror=reject;document.head.append(script)});quickgenNotesCapacity.run({set:async value=>{await switchNotes(value);if(notesMode.active!==value)throw Error('notes mode transition failed');},active:()=>notesMode.active});}
if(boardRehearsal)quickgenBoardRehearsal.run({durable:()=>durable,restoreError:()=>storageError});
if(['notes-rich-smoke','notes-rich-capacity'].includes(diagnostic)&&(await invoke('storage_status')).isolated){await new Promise((resolve,reject)=>{const script=document.createElement('script');script.src='notes-rich-validation.js';script.onload=resolve;script.onerror=reject;document.head.append(script)});quickgenNotesRichValidation.run({set:async value=>{await switchNotes(value);if(notesMode.active!==value)throw Error('notes mode transition failed');},ink:async value=>{await switchInk(value);if(inkTools.active!==value)throw Error('ink mode transition failed');}},diagnostic);}

})().catch(error=>{document.body.inert=false;const el=document.getElementById('notice');el.hidden=false;el.textContent='画板启动失败：'+String(error)});


