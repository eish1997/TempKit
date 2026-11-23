(function(root){
'use strict';
function zoomAt(camera,x,y,delta){const scale=Math.max(.12,Math.min(3,camera.scale*Math.exp(-delta*.0015))),ratio=scale/camera.scale;return{x:x-(x-camera.x)*ratio,y:y-(y-camera.y)*ratio,scale};}
function fit(count,width,height){const columns=Math.max(1,Math.ceil(Math.sqrt(count*1.4))),rows=Math.ceil(count/columns),w=columns*248-28,h=rows*238-28;const scale=Math.min(1,Math.max(.12,Math.min(Math.max(1,width-64)/Math.max(w,1),Math.max(1,height-64)/Math.max(h,1))));return{columns,x:(width-w*scale)/2,y:(height-h*scale)/2,scale};}
function mount({layer,onToggle,onError}){
 const grid=layer.firstElementChild;let camera={x:0,y:0,scale:1},ready=false,pan=null,suppress=false;
 const paint=()=>grid.style.transform=`translate(${camera.x}px,${camera.y}px) scale(${camera.scale})`;
 function render(notes){grid.replaceChildren();for(const note of notes){const tile=document.createElement('button');tile.className='note-tile';tile.type='button';tile.textContent=note.text;tile.classList.toggle('not-resident',!note.resident);tile.setAttribute('aria-pressed',String(note.resident));tile.setAttribute('aria-label',(note.text||'空白便签')+'，'+(note.resident?'常驻桌面':'不常驻'));tile.onclick=async()=>{tile.disabled=true;try{await onToggle(note)}catch(e){onError(e)}finally{tile.disabled=false}};grid.append(tile);}if(!ready){const fitted=fit(notes.length,layer.clientWidth,layer.clientHeight);grid.style.gridTemplateColumns=`repeat(${fitted.columns},220px)`;camera=fitted;ready=true;}paint();}
 layer.addEventListener('pointerdown',e=>{if(e.button!==0&&e.button!==1)return;e.preventDefault();suppress=false;pan={id:e.pointerId,x:e.clientX,y:e.clientY,cx:camera.x,cy:camera.y,moved:false};});
 layer.addEventListener('pointermove',e=>{if(!pan||pan.id!==e.pointerId)return;const dx=e.clientX-pan.x,dy=e.clientY-pan.y;if(!pan.moved&&Math.hypot(dx,dy)<5)return;pan.moved=true;layer.setPointerCapture(e.pointerId);camera={...camera,x:pan.cx+dx,y:pan.cy+dy};paint();});
 const end=e=>{if(!pan||pan.id!==e.pointerId)return;suppress=pan.moved;pan=null;if(layer.hasPointerCapture(e.pointerId))layer.releasePointerCapture(e.pointerId);};
 layer.addEventListener('pointerup',end);layer.addEventListener('pointercancel',end);
 layer.addEventListener('click',e=>{if(suppress){e.preventDefault();e.stopImmediatePropagation();suppress=false;}},true);
 layer.addEventListener('wheel',e=>{e.preventDefault();const rect=layer.getBoundingClientRect();camera=zoomAt(camera,e.clientX-rect.left,e.clientY-rect.top,e.deltaY*(e.deltaMode===1?16:e.deltaMode===2?rect.height:1));paint();},{passive:false});
 return{render,reset(){ready=false;},cancel(){if(pan&&layer.hasPointerCapture(pan.id))layer.releasePointerCapture(pan.id);pan=null;suppress=false;}};
}
function createMode({list,flush,present,release,render,changed,empty}){
 let active=false,tail=Promise.resolve();
 function set(value){const work=tail.then(async()=>{
  if(value===active)return;
  if(value){let notes=(await list()).filter(n=>!n.deleted);if(!notes.length){empty();return;}
   try{await flush();notes=(await list()).filter(n=>!n.deleted);if(!notes.length){await release();empty();return;}await present(true);render(notes);active=true;changed(true);}
   catch(error){try{await present(false);await release();}catch(recovery){active=true;render(notes);changed(true);throw Error(String(error)+'；恢复窗口失败：'+recovery);}throw error;}
  }else{await present(false);await release();active=false;changed(false);}
 });tail=work.catch(()=>{});return work;}
 return{set,get active(){return active;}};
}
const api={zoomAt,fit,mount,createMode};if(typeof module==='object')module.exports=api;else root.quickgenNotesOverview=api;
})(typeof window==='object'?window:globalThis);
