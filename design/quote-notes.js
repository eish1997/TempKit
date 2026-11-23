(()=>{
 document.getElementById('notes-full-motion').addEventListener('change',e=>document.body.classList.toggle('notes-full-motion',e.target.checked));
 const key='quickgen-quotes-preview-v1',nodes=new Map();let notes=[];
 const button=(label,icon)=>{const b=document.createElement('button');b.type='button';b.title=label;b.setAttribute('aria-label',label);b.innerHTML=quickgenIcons[icon]||label;return b;};
 const entry=button('便签','copy');entry.id='notes-entry';entry.innerHTML='<span class="note-stack-icon" aria-hidden="true"><i></i><i class="note-front"><svg class="note-writing" viewBox="0 0 33 35" fill="none"><g class="note-rules"><path d="M6 11H27 M6 18H27 M6 25H27"/></g><g class="note-ink"><path d="M6 10c2-4 3 3 5-1s2 3 4 0 1-3 2-1 2 2 3 0 2 1 3 0 2 0 3-1"/><path d="M6 17c2 1 2-4 3-2s0 4 2 2 2-3 3-1 2 1 3-1 1 4 3 1 2 2 4 0"/><path d="M6 24c1-3 3 2 4-1s2 2 3 0 2-2 3 0 2 1 3-1 2 2 4 0"/></g></svg></i></span>';const tools=document.createElement('div');tools.id='panel-bottom-tools';tools.setAttribute('role','toolbar');tools.setAttribute('aria-label','面板工具');tools.append(entry);document.getElementById('panel').append(tools);document.getElementById('dock').append(document.getElementById('receipt'));
 const pen=document.createElement('button');pen.type='button';pen.id='screen-pen';pen.setAttribute('aria-label','在屏幕上画画，Ctrl+Z 撤销，右键清除');pen.setAttribute('aria-pressed','false');pen.innerHTML='<span class="screen-pen-icon" aria-hidden="true"><svg viewBox="0 0 28 16"><path fill="#f4f4f6" d="M0 3h12v10H2a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z"/><path fill="#e2e2e6" d="M0 3h5v10H2a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z"/><path fill="#1c1c1f" d="M10 1.2 26.8 8 10 14.8z"/><path fill="#4a4a4f" d="M12 4.4 20.2 8 12 6.2z"/></svg></span>';tools.append(pen);
 const ink=document.createElementNS('http://www.w3.org/2000/svg','svg');ink.id='screen-ink';ink.setAttribute('aria-hidden','true');document.body.append(ink);
 let drawing=false,stroke=null;
 function undoStroke(){if(stroke){stroke.path.remove();stroke=null;return;}const last=ink.lastElementChild;if(last)last.remove();}
 function setPen(on){drawing=on;pen.setAttribute('aria-pressed',String(on));document.body.classList.toggle('screen-drawing',on);if(!on)stroke=null;}
 function paintStroke(){const pts=stroke.points;if(pts.length<2){stroke.path.setAttribute('d',`M ${pts[0].x} ${pts[0].y} l .01 .01`);return;}let d=`M ${pts[0].x.toFixed(1)} ${pts[0].y.toFixed(1)}`;for(let i=1;i<pts.length-1;i++){const mx=(pts[i].x+pts[i+1].x)/2,my=(pts[i].y+pts[i+1].y)/2;d+=` Q ${pts[i].x.toFixed(1)} ${pts[i].y.toFixed(1)} ${mx.toFixed(1)} ${my.toFixed(1)}`;}const last=pts[pts.length-1];stroke.path.setAttribute('d',d+` L ${last.x.toFixed(1)} ${last.y.toFixed(1)}`);}
 pen.onclick=()=>setPen(!drawing);
 pen.oncontextmenu=e=>{e.preventDefault();e.stopPropagation();stroke=null;ink.replaceChildren();};
 window.addEventListener('pointerdown',e=>{if(!drawing||e.button!==0||e.target.closest('#screen-pen'))return;e.preventDefault();e.stopPropagation();const path=document.createElementNS('http://www.w3.org/2000/svg','path');ink.append(path);stroke={path,points:[{x:e.clientX,y:e.clientY}],id:e.pointerId};paintStroke();},true);
 window.addEventListener('pointermove',e=>{if(!stroke||e.pointerId!==stroke.id)return;const last=stroke.points.at(-1);if(Math.hypot(e.clientX-last.x,e.clientY-last.y)<1.2)return;stroke.points.push({x:e.clientX,y:e.clientY});paintStroke();},true);
 window.addEventListener('pointerup',e=>{if(stroke?.id===e.pointerId)stroke=null;},true);
 window.addEventListener('pointercancel',e=>{if(stroke?.id===e.pointerId)stroke=null;},true);
 window.addEventListener('contextmenu',e=>{if(drawing&&!e.target.closest('#screen-pen'))e.preventDefault();},true);
 window.addEventListener('keydown',e=>{if(!(e.ctrlKey||e.metaKey)||e.shiftKey||e.key.toLowerCase()!=='z')return;const typing=!drawing&&e.target&&(e.target.isContentEditable||/^(TEXTAREA|INPUT)$/.test(e.target.tagName));if(typing||(!stroke&&!ink.lastElementChild))return;e.preventDefault();e.stopPropagation();undoStroke();},true);
 let overview=false;
 const overviewLayer=document.createElement('section');overviewLayer.id='notes-overview';overviewLayer.hidden=true;overviewLayer.setAttribute('aria-label','整理桌面便签');
 overviewLayer.innerHTML='<div class="notes-grid"></div>';
 const grid=overviewLayer.firstElementChild;
 const camera={x:0,y:0,scale:1};let cameraReady=false,pan=null,suppressClick=false;
 function renderCamera(){grid.style.transform=`translate(${camera.x}px,${camera.y}px) scale(${camera.scale})`;}
 function fitNotes(){const columns=Math.max(1,Math.ceil(Math.sqrt(notes.length*1.4))),rows=Math.ceil(notes.length/columns);grid.style.gridTemplateColumns=`repeat(${columns},220px)`;const w=columns*248-28,h=rows*238-28,available=Math.max(220,innerWidth-320);camera.scale=Math.min(1,Math.max(.15,Math.min((available-64)/Math.max(w,1),(innerHeight-120)/Math.max(h,1))));camera.x=(available-w*camera.scale)/2;camera.y=(overviewLayer.clientHeight-h*camera.scale)/2;renderCamera();cameraReady=true;}
 overviewLayer.addEventListener('pointerdown',e=>{if(e.button!==0&&e.button!==1)return;e.preventDefault();suppressClick=false;pan={id:e.pointerId,x:e.clientX,y:e.clientY,cx:camera.x,cy:camera.y,moved:false};});
 overviewLayer.addEventListener('pointermove',e=>{if(!pan||e.pointerId!==pan.id)return;const dx=e.clientX-pan.x,dy=e.clientY-pan.y;if(!pan.moved&&Math.hypot(dx,dy)<5)return;if(!pan.moved){pan.moved=true;overviewLayer.setPointerCapture(e.pointerId);overviewLayer.classList.add('panning');}camera.x=pan.cx+dx;camera.y=pan.cy+dy;renderCamera();});
 function endPan(e){if(!pan||pan.id!==e.pointerId)return;suppressClick=pan.moved;pan=null;overviewLayer.classList.remove('panning');if(overviewLayer.hasPointerCapture(e.pointerId))overviewLayer.releasePointerCapture(e.pointerId);}
 overviewLayer.addEventListener('pointerup',endPan);overviewLayer.addEventListener('pointercancel',endPan);
 overviewLayer.addEventListener('click',e=>{if(suppressClick){e.preventDefault();e.stopImmediatePropagation();suppressClick=false;}},true);
 overviewLayer.addEventListener('wheel',e=>{e.preventDefault();const rect=overviewLayer.getBoundingClientRect(),x=e.clientX-rect.left,y=e.clientY-rect.top,delta=e.deltaY*(e.deltaMode===1?16:e.deltaMode===2?rect.height:1),next=Math.max(.12,Math.min(3,camera.scale*Math.exp(-delta*.0015))),ratio=next/camera.scale;camera.x=x-(x-camera.x)*ratio;camera.y=y-(y-camera.y)*ratio;camera.scale=next;renderCamera();},{passive:false});
 document.body.append(overviewLayer);
 function message(text,undo){window.dispatchEvent(new CustomEvent('quickgen-panel-notice',{detail:{text,undo}}));}
 function save(){try{localStorage.setItem(key,JSON.stringify(notes));}catch{message('浏览器未能保存便签，请先复制内容。');}}
 function clamp(n){n.w=Math.min(n.w,Math.max(220,innerWidth-16));n.h=Math.min(n.h,Math.max(160,innerHeight-70));n.x=Math.max(8,Math.min(n.x,innerWidth-n.w-8));n.y=Math.max(60,Math.min(n.y,innerHeight-n.h-8));}
 function place(n,el){clamp(n);Object.assign(el.style,{left:n.x+'px',top:n.y+'px',width:n.w+'px',height:n.h+'px'});}
 function lift(n,el){for(const other of notes){const node=nodes.get(other.id);if(node)node.style.zIndex=String(other.pinned?60:30);}el.style.zIndex=String(n.pinned?61:31);}
 function paintTile(n,tile){tile.dataset.theme=n.theme||'';tile.classList.toggle('not-resident',n.hidden);tile.setAttribute('aria-pressed',String(!n.hidden));tile.setAttribute('aria-label',`${n.text||'空白便签'}，${n.hidden?'不常驻':'常驻桌面'}`);}
 function list(){if(!overview)return;grid.replaceChildren();for(const n of notes){const tile=document.createElement('button');tile.type='button';tile.className='note-tile';const content=document.createElement('span');content.className='note-tile-content';content.textContent=n.text;tile.append(content);paintTile(n,tile);tile.onclick=()=>{n.hidden=!n.hidden;paintTile(n,tile);save();};grid.append(tile);}if(!cameraReady)fitNotes();}
 function setOverview(value){overview=value;overviewLayer.hidden=!value;document.body.classList.toggle('notes-organizing',value);entry.setAttribute('aria-expanded',String(value));entry.setAttribute('aria-pressed',String(value));for(const n of notes){const el=nodes.get(n.id);el.hidden=value||n.hidden;el.querySelector('.note-menu').hidden=true;}if(value){list();overviewLayer.querySelector('button')?.focus({preventScroll:true});}else{entry.focus();}}
 function toggle(){if(!notes.length){message('右键便签按钮，新建便签');return;}setOverview(!overview);}
 entry.removeAttribute('title');entry.onclick=toggle;
 entry.oncontextmenu=e=>{e.preventDefault();if(overview)setOverview(false);window.dispatchEvent(new CustomEvent('quickgen-panel-notice',{detail:null}));create();};
 window.addEventListener('keydown',e=>{if(e.key!=='Escape')return;if(drawing){e.preventDefault();e.stopImmediatePropagation();setPen(false);return;}if(overview){e.preventDefault();e.stopImmediatePropagation();setOverview(false);}},true);
 document.querySelector('[data-desktop-app="便签"]').onclick=toggle; function mount(n){const el=document.createElement('section');el.className='desktop-note';el.setAttribute('aria-label','桌面便签');el.dataset.noteId=n.id;el.hidden=overview||n.hidden;
 const head=document.createElement('header'),grip=document.createElement('span');grip.className='note-grip';grip.innerHTML='<svg viewBox="0 0 24 24" aria-hidden="true"><g fill="currentColor" stroke="none"><circle cx="9" cy="6" r="1.4"/><circle cx="15" cy="6" r="1.4"/><circle cx="9" cy="12" r="1.4"/><circle cx="15" cy="12" r="1.4"/><circle cx="9" cy="18" r="1.4"/><circle cx="15" cy="18" r="1.4"/></g></svg>';const pin=button('置顶便签','pin'),menu=button('更多便签操作'),hide=button('收起便签','x');pin.innerHTML='<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 4h8l-1 8 3 3H6l3-3-1-8ZM12 15v5"/></svg>';menu.innerHTML='<svg viewBox="0 0 24 24" aria-hidden="true"><g fill="currentColor" stroke="none"><circle cx="5" cy="12" r="1.4"/><circle cx="12" cy="12" r="1.4"/><circle cx="19" cy="12" r="1.4"/></g></svg>';hide.className='note-close';hide.innerHTML='<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18"/></svg>';head.append(grip,pin,menu,hide);
 const input=document.createElement('textarea');input.setAttribute('aria-label','便签内容');input.placeholder='记点什么…';input.value=n.text;input.spellcheck=false;
 const actions=document.createElement('div');actions.className='note-menu';actions.hidden=true;const del=button('删除便签');del.textContent='删除便签';actions.append(del);
 const resize=button('调整便签大小');resize.className='note-resize';resize.textContent='';el.append(head,input,actions,resize);document.body.append(el);nodes.set(n.id,el);place(n,el);lift(n,el);
 function paintPin(){pin.setAttribute('aria-pressed',String(n.pinned));el.classList.toggle('note-pinned',n.pinned);}paintPin();
 pin.onclick=()=>{n.pinned=!n.pinned;paintPin();lift(n,el);save();};menu.onclick=()=>actions.hidden=!actions.hidden;
 hide.onclick=()=>{n.hidden=true;el.hidden=true;actions.hidden=true;save();list();};
 del.onclick=()=>{notes=notes.filter(v=>v!==n);el.remove();nodes.delete(n.id);save();list();message('便签已删除',()=>{notes.push(n);mount(n);save();list();});};
 input.oninput=()=>{n.text=input.value;save();};
 el.addEventListener('pointerdown',()=>lift(n,el));el.addEventListener('keydown',e=>{if(e.key==='Escape'){actions.hidden=true;e.stopPropagation();}});
 function drag(e,sizing){if(e.button!==0)return;if(!sizing&&e.target.closest('button'))return;e.preventDefault();const target=e.currentTarget,origin={x:n.x,y:n.y,w:n.w,h:n.h},sx=e.clientX,sy=e.clientY;target.setPointerCapture(e.pointerId);
 const move=ev=>{if(sizing){n.w=Math.max(220,origin.w+ev.clientX-sx);n.h=Math.max(160,origin.h+ev.clientY-sy);}else{n.x=origin.x+ev.clientX-sx;n.y=origin.y+ev.clientY-sy;}place(n,el);};const end=()=>{target.removeEventListener('pointermove',move);target.removeEventListener('pointerup',end);target.removeEventListener('pointercancel',end);save();};target.addEventListener('pointermove',move);target.addEventListener('pointerup',end);target.addEventListener('pointercancel',end);}
 head.onpointerdown=e=>drag(e,false);resize.onpointerdown=e=>drag(e,true);
 resize.onkeydown=e=>{if(!e.key.startsWith('Arrow'))return;e.preventDefault();n.w+=e.key==='ArrowRight'?10:e.key==='ArrowLeft'?-10:0;n.h+=e.key==='ArrowDown'?10:e.key==='ArrowUp'?-10:0;n.w=Math.max(220,n.w);n.h=Math.max(160,n.h);place(n,el);save();};
 window.mountQuoteEditor(el,n,save);return el;}
 function create(){const offset=(notes.length%5)*24,n={id:crypto.randomUUID(),text:'',x:Math.min(190,innerWidth/5)+offset,y:130+offset,w:300,h:250,pinned:false,hidden:false};notes.push(n);cameraReady=false;const el=mount(n);save();list();el.querySelector('[contenteditable]').focus();}

 try{const stored=JSON.parse(localStorage.getItem(key)||JSON.stringify(window.quoteDemoNotes));if(Array.isArray(stored))notes=stored.filter(n=>n&&typeof n.id==='string'&&typeof n.text==='string'&&['x','y','w','h'].every(k=>Number.isFinite(n[k])));}catch{message('未能读取之前的便签。');}
 notes.forEach(mount);window.addEventListener('resize',()=>{notes.forEach(n=>place(n,nodes.get(n.id)));save();});
})();






