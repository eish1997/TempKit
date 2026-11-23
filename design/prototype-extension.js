(() => {
 const fullMotion=new URLSearchParams(location.search).get('motion')==='full';
 document.documentElement.dataset.motion=fullMotion?'full':'system';

 const uiIcon=name=>window.quickgenIcons[name].replace('<svg','<svg aria-hidden="true" focusable="false" class="ui-icon"');
 document.querySelectorAll('.close').forEach(node=>node.innerHTML=uiIcon('x'));
 document.querySelectorAll('.mark').forEach(node=>node.outerHTML=uiIcon('panel-top'));
 document.querySelector('#copy').innerHTML=uiIcon('copy')+'<span>复制</span>';
 const presets = Array.from(document.querySelector('.grid').children).map((a,i)=>({id:'preset-'+i,name:a.dataset.card,prompt:a.title,art:a.querySelector('svg').outerHTML}));
 let owned=presets.slice(0,2), cover='', edge='top', fraction=.5, suppress=false;
 const editor=document.createElement('dialog');editor.className='editor dark';
 editor.innerHTML='<form><div class="dialog-head"><h3>自定义能力</h3><button type="button" data-cancel aria-label="关闭">'+uiIcon('x')+'</button></div><p class="hint">把常用的处理要求保存成一张卡片。</p><label>能力名称<input name="name" required maxlength="24" placeholder="例如：产品柔光棚拍"></label><label>处理指令<textarea name="prompt" required maxlength="2000" placeholder="描述希望 AI 如何处理拖入的图片，例如：保持物体形状与构图，替换为柔和棚拍灯光，背景为浅灰色。"></textarea></label><label>卡片封面（可选）<input name="cover" type="file" accept="image/png,image/jpeg,image/webp"></label><p class="field-error" id="cover-note" role="alert"></p><div class="actions"><button type="button" data-cancel>取消</button><button type="submit">添加到面板</button></div></form>';
 const library=document.createElement('dialog');library.className='library dark';
 document.body.append(editor,library);
 let attached=null;
 function placeAttached(){
 if(!attached?.dialog.open)return;
 const {dialog,anchor}=attached,r=anchor.getBoundingClientRect();
 const bounds={left:6,top:6,right:innerWidth-6,bottom:innerHeight-6};
 const gap=6,width=Math.min(308,bounds.right-bounds.left);
 dialog.style.width=width+'px';dialog.style.maxHeight=(bounds.bottom-bounds.top)+'px';
 const height=dialog.getBoundingClientRect().height;
 let x=r.right+gap,y=r.top;
 if(x+width>bounds.right){if(r.left-gap-width>=bounds.left)x=r.left-gap-width;else{ x=r.left;y=r.bottom+gap;if(y+height>bounds.bottom)y=r.top-height-gap; }}
 x=Math.max(bounds.left,Math.min(x,bounds.right-width));y=Math.max(bounds.top,Math.min(y,bounds.bottom-height));
 dialog.style.left=x+'px';dialog.style.top=y+'px';
 }
 function openAttached(dialog,anchor){[editor,library].forEach(other=>{if(other.open)other.close()});attached={dialog,anchor};dialog.classList.add('attached-dialog');dialog.show();placeAttached()}
 [editor,library].forEach(dialog=>{dialog.addEventListener('close',()=>{if(attached?.dialog===dialog)attached=null});new ResizeObserver(()=>{if(attached?.dialog===dialog)placeAttached()}).observe(dialog)});
 document.addEventListener('keydown',e=>{if(e.key==='Escape'&&attached){attached.dialog.close();e.preventDefault()}});
 window.addEventListener('resize',placeAttached);window.addEventListener('scroll',placeAttached,true);

 document.querySelectorAll('.panel .bar').forEach(bar=>{bar.querySelector('.brand')?.remove();if(!bar.parentElement.querySelector('.grid'))return;bar.insertAdjacentHTML('afterbegin','<button class="icon" data-create title="自定义添加" aria-label="自定义添加">'+uiIcon('plus')+'</button><button class="icon" data-library title="预设库" aria-label="预设库">'+uiIcon('library')+'</button><button class="icon height-toggle" aria-label="上下拖动调整卡片高度" title="上下拖动调整卡片高度">'+uiIcon('arrow-up-down')+'</button><span class="bar-space"></span>')});
 function render(){document.querySelectorAll('.grid').forEach(grid=>{grid.replaceChildren();owned.forEach(item=>{const a=document.createElement('a');a.className='card';a.href=grid.closest('.light')?'#panel-light-result':'#panel-dark-result';a.dataset.card=item.name;a.title=item.prompt;if(item.cover){const img=document.createElement('img');img.src=item.cover;img.alt='';a.append(img)}else a.innerHTML=item.art||presets[0].art;const span=document.createElement('span');span.className='card-title';span.textContent=item.name;a.append(span);grid.append(a)})})}
 function renderLibrary(){library.innerHTML='<div class="dialog-head"><h3>预设库</h3><button data-cancel aria-label="关闭预设库">'+uiIcon('x')+'</button></div><p class="hint">添加常用能力到你的面板。</p>';presets.forEach(item=>{const row=document.createElement('div');row.className='preset-row';const info=document.createElement('div');const title=document.createElement('strong');title.textContent=item.name;const desc=document.createElement('p');desc.textContent=item.prompt;info.append(title,desc);const btn=document.createElement('button');const added=owned.some(x=>x.id===item.id);btn.innerHTML=uiIcon(added?'check':'plus')+'<span>'+(added?'已添加':'添加')+'</span>';btn.disabled=added;btn.onclick=()=>{owned.push(item);render();renderLibrary()};row.append(info,btn);library.append(row)})}
 document.addEventListener('click',e=>{if(e.target.closest('[data-create]')){editor.querySelector('form').reset();cover='';editor.querySelector('#cover-note').textContent='';editor.classList.toggle('light',!!e.target.closest('.light'));editor.classList.toggle('dark',!e.target.closest('.light'));openAttached(editor,e.target.closest('.panel'))}if(e.target.closest('[data-library]')){renderLibrary();library.classList.toggle('light',!!e.target.closest('.light'));library.classList.toggle('dark',!e.target.closest('.light'));openAttached(library,e.target.closest('.panel'))}if(e.target.closest('[data-cancel]'))e.target.closest('dialog').close();const card=e.target.closest('.card');if(card)document.querySelectorAll('.status').forEach(x=>x.textContent=card.dataset.card+' · 已保存');},true);
 editor.querySelector('[name=cover]').onchange=async e=>{const file=e.target.files[0];cover='';if(!file)return;if(!['image/png','image/jpeg','image/webp'].includes(file.type)||file.size>5*1024*1024){e.target.value='';editor.querySelector('#cover-note').textContent='请选择 5 MB 以内的 PNG、JPG 或 WebP。';return}cover=await new Promise(resolve=>{const reader=new FileReader();reader.onload=()=>resolve(reader.result);reader.readAsDataURL(file)});editor.querySelector('#cover-note').textContent=''};
 editor.querySelector('form').onsubmit=e=>{e.preventDefault();const form=e.target;const name=form.elements.name.value.trim(),prompt=form.elements.prompt.value.trim();if(!name||!prompt)return;owned.push({id:'custom-'+Date.now(),name,prompt,cover});render();editor.close();showNote('已添加到面板')};
 const clamp=(v,min,max)=>Math.max(min,Math.min(v,max));
 const flows=[];
 const imageFile=file=>file && /\.(png|jpe?g|webp)$/i.test(file.name) && (!file.type || file.type==='application/octet-stream' || ['image/png','image/jpeg','image/webp'].includes(file.type));
 // Native file drags may hide MIME types/items until drop. Never reject an
 // unknown file during hover: validate the real File after it is released.
 const imageDrag=dt=>{
 const files=Array.from(dt?.items||[]).filter(item=>item.kind==='file');
 if(files.length)return files.some(item=>!item.type || item.type==='application/octet-stream' || ['image/png','image/jpeg','image/webp'].includes(item.type));
 return Array.from(dt?.types||[]).includes('Files') || !!dt?.files?.length;
 };

 document.querySelectorAll('.desktop').forEach(d=>{
 const old=d.querySelector('.shelf'), source=document.querySelector('#panel-dark-default .panel');
 const shell=document.createElement('div');shell.className='morph-shell dark';shell.innerHTML='<div class="morph-surface"></div><button class="morph-trigger" aria-label="展开面板" title="点击展开，拖动贴边">'+uiIcon('panel-top')+'</button>';
 const content=source.cloneNode(true);content.classList.add('morph-content');shell.append(content);old.replaceWith(shell);
 const state={d,shell,content,open:d.closest('.frame').id==='flow-open',edge:'top',fraction:.5,width:280,height:220,pending:null,pendingUrl:null,entryDrag:null};flows.push(state);
 function layout(){shell.dataset.edge=state.edge;const w=d.clientWidth,h=d.clientHeight,pw=Math.min(state.width,w),ph=Math.min(state.height,h);const top=state.edge==='top';const nx=top?clamp(w*state.fraction-44,0,w-88):state.edge==='left'?0:w-18;const ny=top?0:clamp(h*state.fraction-44,0,h-88);const x=top?clamp(nx+44-pw/2,0,w-pw):state.edge==='left'?0:w-pw;const y=top?0:clamp(ny+44-ph/2,0,h-ph);Object.assign(shell.style,{left:x+'px',top:y+'px',width:pw+'px',height:ph+'px'});shell.style.setProperty('--closed-clip','inset('+(ny-y)+'px '+(pw-(nx-x)-(top?88:18))+'px '+(ph-(ny-y)-(top?18:88))+'px '+(nx-x)+'px round 9px)');const trigger=shell.querySelector('.morph-trigger');Object.assign(trigger.style,{left:nx-x+'px',top:ny-y+'px',width:(top?88:18)+'px',height:(top?18:88)+'px'});shell.classList.toggle('expanded',state.open);content.inert=!state.open;trigger.hidden=false;trigger.tabIndex=state.open?-1:0;trigger.setAttribute('aria-hidden',String(state.open));}
 state.layout=layout;state.setOpen=v=>{
 if(state.open===v)return;
 // Commit the current shape before retargeting, including after a hidden
 // demo frame becomes visible. CSS transitions reverse from their current value.
 shell.classList.remove('manipulating');
 shell.getBoundingClientRect();getComputedStyle(shell).clipPath;
 state.open=v;if(!v&&attached?.anchor===content)attached.dialog.close();layout();
 };
 shell.querySelector('.morph-trigger').onclick=()=>{if(!suppress)state.setOpen(true)};
 const close=content.querySelector('.close');
 const backIcon=uiIcon('arrow-left');
 function restoreCards(){content.querySelector('.local-result')?.remove();content.querySelector('.grid').hidden=false;content.querySelector('.bar-space').textContent='';close.innerHTML=uiIcon('x');close.setAttribute('aria-label','收起面板');close.title='收起面板'}
 close.onclick=e=>{e.preventDefault();if(content.querySelector('.local-result')){restoreCards();return}state.setOpen(false)};
 const handles=['n','s','e','w','ne','nw','se','sw'].map(dir=>{const handle=document.createElement('button');handle.className='edge-resize edge-'+dir;handle.dataset.dir=dir;handle.title='拖动调整尺寸';handle.setAttribute('aria-label','调整 '+dir+' 边尺寸，支持方向键');content.append(handle);return handle});
 let drag=null;
 shell.onpointerdown=e=>{const resize=e.target.closest('.edge-resize');if(!resize&&(!e.target.closest('.morph-trigger,.bar')||e.target.closest('.bar button,.bar a')))return;drag={x:e.clientX,y:e.clientY,w:state.width,h:state.height,resize:resize?.dataset.dir,moved:false};e.target.setPointerCapture(e.pointerId)};
 shell.onpointermove=e=>{if(!drag)return;if(Math.hypot(e.clientX-drag.x,e.clientY-drag.y)<5&&!drag.moved)return;drag.moved=true;if(drag.resize){const dx=e.clientX-drag.x,dy=e.clientY-drag.y;state.width=clamp(drag.w+(drag.resize.includes('e')?dx:drag.resize.includes('w')?-dx:0),120,d.clientWidth);state.height=clamp(drag.h+(drag.resize.includes('s')?dy:drag.resize.includes('n')?-dy:0),120,d.clientHeight)}else{const r=d.getBoundingClientRect(),x=e.clientX-r.left,y=e.clientY-r.top;state.edge=y<Math.min(x,r.width-x)?'top':x<r.width/2?'left':'right';state.fraction=state.edge==='top'?x/r.width:y/r.height;}shell.classList.add('manipulating');layout();placeAttached()};
 shell.onpointerup=()=>{suppress=!!drag?.moved;drag=null;shell.classList.remove('manipulating');setTimeout(()=>suppress=false,0)};shell.onpointercancel=()=>{drag=null;shell.classList.remove('manipulating')};
 handles.forEach(handle=>handle.onkeydown=e=>{if(!['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(e.key))return;e.preventDefault();state.width=clamp(state.width+(e.key==='ArrowRight'?16:e.key==='ArrowLeft'?-16:0),120,d.clientWidth);state.height=clamp(state.height+(e.key==='ArrowDown'?16:e.key==='ArrowUp'?-16:0),120,d.clientHeight);layout()});
 function keepImage(file){
 if(state.pendingUrl)URL.revokeObjectURL(state.pendingUrl);
 state.pending=file;state.pendingUrl=URL.createObjectURL(file);
 let thumb=content.querySelector('.pending-image');if(!thumb){thumb=document.createElement('img');thumb.className='pending-image';content.querySelector('.bar-space').before(thumb)}
 thumb.src=state.pendingUrl;thumb.alt='待处理图片：'+file.name;
 }
 function runCard(card){
 content.querySelector('.grid').hidden=true;let result=content.querySelector('.local-result');if(!result){result=document.createElement('div');result.className='local-result';content.append(result)}
 result.innerHTML=document.querySelector('.result-shot').outerHTML;
 // Prototype only: retain the actual input for the future model request.
 result.inputFile=state.pending;result.dataset.inputName=state.pending?.name||'';
 content.querySelector('.pending-image')?.remove();state.pending=null;
 if(state.pendingUrl){URL.revokeObjectURL(state.pendingUrl);state.pendingUrl=null}
 content.querySelector('.bar-space').textContent=card.dataset.card+' · 已保存';close.innerHTML=backIcon;close.setAttribute('aria-label','返回上一级');
 }
 shell.ondragenter=shell.ondragover=e=>{
 e.preventDefault();e.dataTransfer.dropEffect=imageDrag(e.dataTransfer)?'copy':'none';
 if(!imageDrag(e.dataTransfer))return;
 if(!state.open){state.entryDrag={x:e.clientX,y:e.clientY,rect:shell.querySelector('.morph-trigger').getBoundingClientRect(),moved:false};restoreCards();state.setOpen(true)}
 const entry=state.entryDrag;if(entry){const r=entry.rect;entry.moved=Math.hypot(e.clientX-entry.x,e.clientY-entry.y)>8 && (e.clientX<r.left||e.clientX>r.right||e.clientY<r.top||e.clientY>r.bottom)}
 content.querySelectorAll('.card').forEach(card=>card.classList.toggle('drop-target',card===e.target.closest('.card')&&(!entry||entry.moved)));
 };
 shell.ondrop=e=>{
 e.preventDefault();e.stopPropagation();
 const files=Array.from(e.dataTransfer.files),file=files[0],entry=state.entryDrag;
 state.entryDrag=null;content.querySelectorAll('.drop-target').forEach(el=>el.classList.remove('drop-target'));
 if(files.length!==1||!imageFile(file)){showNote(files.length>1?'请一次拖入一张图片':'请拖入 png、jpg 或 webp');return}
 state.setOpen(true);restoreCards();keepImage(file);
 const card=e.target.closest('.card');if(card&&(!entry||entry.moved))runCard(card);
 };
 shell.ondragleave=e=>{if(!shell.contains(e.relatedTarget))content.querySelectorAll('.drop-target').forEach(el=>el.classList.remove('drop-target'))};
 shell.addEventListener('click',e=>{const card=e.target.closest('.card');if(!card)return;e.preventDefault();e.stopPropagation();runCard(card)});
 layout();
 });
 function resetFileDrag(){flows.forEach(state=>{state.entryDrag=null;state.content.querySelectorAll('.drop-target').forEach(el=>el.classList.remove('drop-target'))})}
 document.addEventListener('dragend',resetFileDrag);document.addEventListener('drop',resetFileDrag);
 window.addEventListener('blur',resetFileDrag);
 window.addEventListener('beforeunload',()=>flows.forEach(state=>{if(state.pendingUrl)URL.revokeObjectURL(state.pendingUrl)}));
 function position(){flows.forEach(f=>f.layout());placeAttached()}
 document.querySelectorAll('#panel-dark-result .close,#panel-light-result .close').forEach(back=>{back.innerHTML=uiIcon('arrow-left');back.setAttribute('aria-label','返回上一级');back.title='返回上一级';back.href=back.closest('.light')?'#panel-light-default':'#panel-dark-default'});

 document.addEventListener('dragover',e=>{if(Array.from(e.dataTransfer?.types||[]).includes('Files')&&!e.target.matches('input[type=file]')){e.preventDefault();e.dataTransfer.dropEffect=e.target.closest('.card,.morph-shell')?'copy':'none'}});
 document.addEventListener('drop',e=>{
 if(e.target.matches('input[type=file]'))return;
 if(Array.from(e.dataTransfer?.types||[]).includes('Files'))e.preventDefault();
 const card=e.target.closest('.card');if(!card)return;e.preventDefault();
 const files=Array.from(e.dataTransfer.files);if(files.length!==1||!imageFile(files[0])){showNote('请拖入一张 png、jpg 或 webp');return}card.click();
 });
 document.querySelectorAll('.panel:not(.morph-content)').forEach(panel=>{['n','s','e','w','ne','nw','se','sw'].forEach(dir=>{const h=document.createElement('button');h.className='edge-resize edge-'+dir;h.setAttribute('aria-label','调整 '+dir+' 边尺寸');panel.append(h);let start;h.onpointerdown=e=>{start={x:e.clientX,y:e.clientY,w:panel.offsetWidth,h:panel.offsetHeight};h.setPointerCapture(e.pointerId)};h.onpointermove=e=>{if(!start)return;panel.style.width=clamp(start.w+(dir.includes('e')?e.clientX-start.x:dir.includes('w')?start.x-e.clientX:0),120,window.innerWidth-56)+'px';panel.style.height=clamp(start.h+(dir.includes('s')?e.clientY-start.y:dir.includes('n')?start.y-e.clientY:0),120,520)+'px'};h.onpointerup=h.onpointercancel=()=>start=null;h.onkeydown=e=>{if(!e.key.startsWith('Arrow'))return;e.preventDefault();panel.style.width=clamp(panel.offsetWidth+(e.key==='ArrowRight'?16:e.key==='ArrowLeft'?-16:0),120,window.innerWidth-56)+'px';panel.style.height=clamp(panel.offsetHeight+(e.key==='ArrowDown'?16:e.key==='ArrowUp'?-16:0),120,520)+'px'}})});
 let heightIndex=2;
 const heights=[32,56,80,112], names=['极矮','矮','中','高'];
 const heightControls=Array.from(document.querySelectorAll('.height-toggle'));
 function updateHeight(){document.documentElement.style.setProperty('--card-height',heights[heightIndex]+'px');heightControls.forEach(control=>{control.setAttribute('aria-valuenow',heightIndex);control.setAttribute('aria-valuetext',names[heightIndex]+' '+heights[heightIndex]+' 像素')})}
 heightControls.forEach(control=>{
 control.setAttribute('role','slider');control.setAttribute('aria-valuemin','0');control.setAttribute('aria-valuemax','3');control.setAttribute('aria-orientation','vertical');
 const icon=control.querySelector('svg');let start=null,animation=null;
 control.onpointerdown=e=>{if(e.button!==0)return;e.stopPropagation();animation?.cancel();icon.style.transform='none';start={y:e.clientY,index:heightIndex};control.classList.add('adjusting');control.setPointerCapture(e.pointerId)};
 control.onpointermove=e=>{if(!start)return;e.stopPropagation();const delta=e.clientY-start.y;heightIndex=clamp(start.index+Math.round(delta/22),0,3);updateHeight();if(!matchMedia('(prefers-reduced-motion: reduce)').matches)icon.style.transform='translateY('+clamp(delta*.15,-3,3)+'px) scale(1.08)'};
 const settle=e=>{if(!start)return;e?.stopPropagation();start=null;control.classList.remove('adjusting');const from=icon.style.transform;icon.style.transform='none';if(!matchMedia('(prefers-reduced-motion: reduce)').matches)animation=icon.animate([{transform:from},{transform:'translateY(-1px) scale(.96)',offset:.55},{transform:'translateY(.5px) scale(1.02)',offset:.8},{transform:'none'}],{duration:280,easing:'cubic-bezier(.23,1,.32,1)'})};
 control.onpointerup=control.onpointercancel=control.onlostpointercapture=settle;
 control.onclick=e=>{e.preventDefault();e.stopPropagation()};
 control.onkeydown=e=>{if(!['ArrowUp','ArrowDown','Home','End'].includes(e.key))return;e.preventDefault();heightIndex=e.key==='Home'?0:e.key==='End'?3:clamp(heightIndex+(e.key==='ArrowDown'?1:-1),0,3);updateHeight()};
 });
 updateHeight();
 const back=document.createElement('a');back.href='#board';back.className='board-link';back.textContent='返回原型总览';document.body.append(back);
 document.querySelector('.notes').insertAdjacentHTML('beforeend','<p>拖动入口可贴靠顶部、左侧或右侧。面板中的卡片由用户添加；演示初始放置两张，其他能力可从预设库添加，也可自定义名称、处理指令和封面。本页仅模拟生成与保存；新增卡片保留到刷新页面。</p>');
 // Strip visual help text from both initial and dynamically rendered views.
 function removeHints(root){
 if(root.nodeType!==1)return;
 if(root.matches('[title]'))root.removeAttribute('title');
 if(root.matches('[placeholder]'))root.removeAttribute('placeholder');
 root.querySelectorAll('[title]').forEach(el=>el.removeAttribute('title'));
 root.querySelectorAll('[placeholder]').forEach(el=>el.removeAttribute('placeholder'));
 root.querySelectorAll('.hint,.lede,.notes,.label').forEach(el=>el.remove());
 }
 removeHints(document.body);
 new MutationObserver(records=>records.forEach(record=>{
 if(record.type==='attributes'){if(record.target.hasAttribute(record.attributeName))record.target.removeAttribute(record.attributeName);return}
 record.addedNodes.forEach(node=>{if(node.nodeType!==1)return;if(node.matches('.hint,.lede,.notes,.label'))node.remove();else removeHints(node)})
 })).observe(document.body,{subtree:true,childList:true,attributes:true,attributeFilter:['title','placeholder']});
 render();position();window.addEventListener('resize',position);window.addEventListener('hashchange',()=>requestAnimationFrame(position));
})();
