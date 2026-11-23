(()=>{
const blocks=[{text:'午后的室内场景\n今天先把光线和材质的方向记下来。'}, {id:'demo-light',quote:true,text:'柔和的侧面自然光，保留木材的细微纹理，阴影干净，不要过度锐化。'}, {text:'先比较两版光照，再决定用哪一版。\n\n参考图里的暖色可以保留，但墙面要更干净。\n\n人物暂时不动，优先把空间关系做清楚。\n\n下面这段留给下一轮材质调整。'}, {id:'demo-material',quote:true,text:'浅橡木、米白色织物和细腻的哑光石材，减少反光，保持自然的材质层次。'}, {text:'完成之后再检查一次画面边缘。'}];
window.quoteDemoNotes=[{id:'quote-demo',text:blocks.map(b=>b.text).join('\n'),blocks,x:130,y:120,w:370,h:460,pinned:false,hidden:false}];
const notify=text=>window.dispatchEvent(new CustomEvent('quickgen-panel-notice',{detail:{text}}));
window.mountQuoteEditor=(el,n,save)=>{
 const old=el.querySelector('textarea'),editor=document.createElement('div');editor.className='quote-editor';editor.contentEditable='true';editor.setAttribute('role','textbox');editor.setAttribute('aria-label','便签正文');editor.setAttribute('aria-multiline','true');editor.spellcheck=false;
 const footer=document.createElement('nav');footer.className='quote-cards';footer.setAttribute('aria-label','引用片段');
 const make=document.createElement('button');make.className='quote-create';make.textContent='引用';make.title='将选中文字设为引用块';make.setAttribute('aria-label','创建引用块');make.hidden=false;
 let selected=null,composing=false,flashTimer;
 function block(text,quote=false,id){const p=document.createElement(quote?'blockquote':'div');p.textContent=text;if(!text)p.append(document.createElement('br'));if(quote)p.dataset.quoteId=id||crypto.randomUUID();return p;}
 const tagStates={pending:'待开始',active:'进行中',rework:'返修中',done:'已完成',paused:'已暂停'};
 function stampTag(tag,created){if(!created||!Number.isFinite(Date.parse(created)))return;tag.dataset.createdAt=new Date(created).toISOString();const d=new Date(created),pad=v=>String(v).padStart(2,'0');tag.title=`创建于 ${d.getFullYear()}年${d.getMonth()+1}月${d.getDate()}日 ${pad(d.getHours())}:${pad(d.getMinutes())}`;}
 function cleanHTML(html){const template=document.createElement('template');template.innerHTML=html;const allowed=new Set(['B','STRONG','SPAN','MARK','BR','DIV','P']);for(const node of [...template.content.querySelectorAll('*')].reverse()){if(!allowed.has(node.tagName)){node.replaceWith(document.createTextNode(node.textContent));continue;}const highlighted=node.style.backgroundColor,todo=node.classList.contains('quote-todo'),done=node.dataset.done,tag=node.classList.contains('quote-tag'),draft=node.classList.contains('quote-tag-draft'),created=node.dataset.createdAt,status=node.dataset.status;for(const attr of [...node.attributes])node.removeAttribute(attr.name);if(tag){node.className='quote-tag';node.contentEditable='false';stampTag(node,created);}if(draft){node.className='quote-tag-draft';if(created&&Number.isFinite(Date.parse(created)))node.dataset.createdAt=created;}if((tag||draft)&&tagStates[status])node.dataset.status=status;if(todo){node.className='quote-todo';node.dataset.done=done==='true'?'true':'false';}if(highlighted&&highlighted!=='transparent')node.style.backgroundColor='#ead584';}return template.content;}
 for(const b of n.blocks||[{text:n.text}]){const node=b.divider?document.createElement('hr'):block(b.text,b.quote,b.id);if(b.html)node.replaceChildren(cleanHTML(b.html));if(b.todo){node.classList.add('quote-todo');node.dataset.done=String(!!b.done);}editor.append(node);}old.replaceWith(editor);
 const toolbar=document.createElement('div');toolbar.className='quote-toolbar';toolbar.setAttribute('role','toolbar');toolbar.setAttribute('aria-label','文本格式');el.append(footer,toolbar);
 const connector=document.createElementNS('http://www.w3.org/2000/svg','svg');connector.classList.add('quote-connector');connector.setAttribute('aria-hidden','true');const connectorPath=document.createElementNS('http://www.w3.org/2000/svg','path');connector.append(connectorPath);el.append(connector);
 function capture(){n.blocks=[...editor.childNodes].map(node=>({text:node.nodeType===3?node.textContent:node.innerText,...(node.nodeType===1?{divider:node.tagName==='HR',html:(node.classList.contains('quote-tag')||node.classList.contains('quote-tag-draft'))?node.outerHTML:node.innerHTML,todo:node.classList.contains('quote-todo'),done:node.dataset.done==='true'}:{}),quote:node.nodeType===1&&node.tagName==='BLOCKQUOTE',...(node.dataset?.quoteId?{id:node.dataset.quoteId}:{})}));n.text=editor.innerText;save();renderCards();}
 let navigationFrame=0,hoverQuote=null,clickedQuote=null,connectorTimer;
 function syncWholeCards(){
  const top=footer.scrollTop+5,bottom=footer.scrollTop+footer.clientHeight-5;
  for(const card of footer.children){const whole=card.offsetTop>=top&&card.offsetTop+card.offsetHeight<=bottom;card.classList.toggle('rail-visible',whole);card.inert=!whole;card.setAttribute('aria-hidden',String(!whole));}
 }

 function syncNavigation(){
  navigationFrame=0;if(!el.isConnected)return;
  const quotes=[...editor.querySelectorAll('blockquote')],bounds=editor.getBoundingClientRect(),line=bounds.top+32;
  let current=-1,distance=Infinity;
  quotes.forEach((q,i)=>{const r=q.getBoundingClientRect();if(r.bottom>bounds.top&&r.top<bounds.bottom){const d=r.top<=line&&r.bottom>=line?0:Math.abs(r.top-line);if(d<distance){distance=d;current=i;}}});
  if(current<0&&quotes.length){current=0;quotes.forEach((q,i)=>{if(q.getBoundingClientRect().top<=line)current=i;});}
  [...footer.children].forEach((tab,i)=>{const active=i===current;tab.classList.toggle('active',active);if(active)tab.setAttribute('aria-current','location');else tab.removeAttribute('aria-current');});
  const active=footer.children[current];if(active){if(active.offsetTop<footer.scrollTop+5)footer.scrollTop=Math.max(0,active.offsetTop-8);else if(active.offsetTop+active.offsetHeight>footer.scrollTop+footer.clientHeight-5)footer.scrollTop=active.offsetTop+active.offsetHeight-footer.clientHeight+8;}
  syncWholeCards();
  connector.style.opacity='0';
  const target=hoverQuote||clickedQuote,index=quotes.indexOf(target),targetTab=footer.children[index];
  if(targetTab?.classList.contains('rail-visible')&&target){const q=target.getBoundingClientRect(),tab=targetTab.getBoundingClientRect(),host=el.getBoundingClientRect(),rail=footer.getBoundingClientRect();const top=Math.max(q.top,bounds.top+4),bottom=Math.min(q.bottom,bounds.bottom-4);if(bottom>top&&tab.bottom>rail.top&&tab.top<rail.bottom){const sx=tab.right-host.left-1,sy=tab.top+tab.height/2-host.top-1,ex=q.left-host.left-1,ey=(top+bottom)/2-host.top-1,mid=(sx+ex)/2;connectorPath.setAttribute('d',`M ${sx} ${sy} C ${mid} ${sy}, ${mid} ${ey}, ${ex} ${ey}`);connectorPath.style.stroke=target.style.borderLeftColor;connector.style.opacity='.55';}}

 }
 function scheduleNavigation(){if(!navigationFrame)navigationFrame=requestAnimationFrame(syncNavigation);}
 editor.addEventListener('scroll',scheduleNavigation,{passive:true});footer.addEventListener('scroll',()=>{syncWholeCards();connector.style.opacity='0';},{passive:true});
 const navigationObserver=new ResizeObserver(scheduleNavigation);navigationObserver.observe(editor);
 const removalObserver=new MutationObserver(()=>{if(!el.isConnected){navigationObserver.disconnect();removalObserver.disconnect();cancelAnimationFrame(navigationFrame);clearTimeout(connectorTimer);}});removalObserver.observe(document.body,{childList:true});
 // Independent, stable paper variations keyed by the quote, not its list position.
 function dressPaper(card,id){
  let seed=2166136261;for(const ch of id)seed=Math.imul(seed^ch.charCodeAt(0),16777619)>>>0;
  const random=()=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed/4294967296;};
  card.style.setProperty('--paper-width',(23+Math.floor(random()*9))+'px');
  card.style.setProperty('--tilt',(-6+random()*12).toFixed(1)+'deg');
  const spacing=random();card.style.setProperty('--space',(spacing<.4?-4+Math.floor(random()*4):spacing<.78?1+Math.floor(random()*4):7+Math.floor(random()*7))+'px');
  const textures=['linear-gradient(110deg,#fff3,transparent 65%)','repeating-linear-gradient(0deg,transparent 0 5px,#70644215 5px 6px)','radial-gradient(#7f745026 .55px,transparent .8px)','repeating-linear-gradient(90deg,transparent 0 6px,#78684210 6px 7px),repeating-linear-gradient(0deg,transparent 0 6px,#78684210 6px 7px)'];
  const texture=Math.floor(random()*textures.length);card.style.setProperty('--paper-texture',textures[texture]);if(texture===2)card.style.setProperty('background-size','4px 4px,auto','important');
  card.style.setProperty('--tab-color',['#ddc98e','#c7cfaa','#d9b99e','#b9ccca'][Math.floor(random()*4)]);
 }
 function renderCards(){footer.replaceChildren();for(const quote of editor.querySelectorAll('blockquote')){if(!quote.dataset.quoteId)quote.dataset.quoteId=crypto.randomUUID();const card=document.createElement('button');card.className='quote-card';card.inert=true;card.draggable=true;card.textContent=Array.from(quote.innerText.trim())[0]||'·';dressPaper(card,quote.dataset.quoteId);quote.style.borderLeftColor=card.style.getPropertyValue('--tab-color');card.onpointerenter=()=>{hoverQuote=quote;scheduleNavigation();};card.onpointerleave=()=>{hoverQuote=null;scheduleNavigation();};card.onfocus=()=>{if(card.matches(':focus-visible')){hoverQuote=quote;scheduleNavigation();}};card.onblur=()=>{hoverQuote=null;scheduleNavigation();};card.setAttribute('aria-label','定位引用：'+(quote.innerText||'空引用'));card.title=quote.innerText;card.onclick=()=>{clickedQuote=quote;clearTimeout(connectorTimer);connectorTimer=setTimeout(()=>{clickedQuote=null;scheduleNavigation();},1200);editor.scrollTo({top:editor.scrollTop+quote.getBoundingClientRect().top-editor.getBoundingClientRect().top-28,behavior:matchMedia('(prefers-reduced-motion: reduce)').matches?'instant':'smooth'});editor.querySelectorAll('.quote-flash').forEach(q=>q.classList.remove('quote-flash'));quote.classList.add('quote-flash');clearTimeout(flashTimer);flashTimer=setTimeout(()=>quote.classList.remove('quote-flash'),1100);scheduleNavigation();};card.ondragstart=e=>{e.dataTransfer.setData('text/plain',quote.innerText);e.dataTransfer.effectAllowed='copy';card.classList.add('dragging');};card.ondragend=()=>card.classList.remove('dragging');card.oncontextmenu=async e=>{e.preventDefault();try{await navigator.clipboard.writeText(quote.innerText);notify('引用已复制');}catch{notify('未能复制，请拖动标签取用文字');}};footer.append(card);}footer.hidden=!footer.children.length;scheduleNavigation();}
 editor.addEventListener('compositionstart',()=>composing=true);editor.addEventListener('compositionend',()=>{composing=false;capture();});editor.addEventListener('input',()=>{if(!composing)capture();});
 editor.addEventListener('paste',e=>{e.preventDefault();const selection=getSelection();if(!selection.rangeCount)return;const range=selection.getRangeAt(0);range.deleteContents();const text=document.createTextNode(e.clipboardData.getData('text/plain'));range.insertNode(text);range.setStartAfter(text);range.collapse(true);selection.removeAllRanges();selection.addRange(range);capture();});
 const buttons={};
 const currentBlock=()=>{let node=getSelection().anchorNode;node=node?.nodeType===3?node.parentElement:node;while(node&&node.parentElement!==editor)node=node.parentElement;return node&&editor.contains(node)?node:null;};
 function syncTools(){const selection=getSelection();if(selection.rangeCount&&editor.contains(selection.anchorNode)&&editor.contains(selection.focusNode))selected=selection.getRangeAt(0).cloneRange();const node=currentBlock();buttons.bold?.setAttribute('aria-pressed',String(!!node&&document.queryCommandState('bold')));buttons.highlight?.setAttribute('aria-pressed',String(!!node&&document.queryCommandValue('backColor')==='rgb(234, 213, 132)'));buttons.todo?.setAttribute('aria-pressed',String(!!node?.classList.contains('quote-todo')));make.setAttribute('aria-pressed',String(node?.tagName==='BLOCKQUOTE'));}
 document.addEventListener('selectionchange',syncTools);
 function restore(){const range=selected?.cloneRange();if(document.activeElement!==editor)editor.focus();if(range&&editor.contains(range.commonAncestorContainer)){const selection=getSelection();selection.removeAllRanges();selection.addRange(range);selected=range.cloneRange();}}
 function tool(key,label,icon,action){const button=document.createElement('button');button.type='button';button.title=label;button.setAttribute('aria-label',label);button.setAttribute('aria-pressed','false');button.innerHTML=icon;button.dataset.tool=key;button.onpointerdown=e=>e.preventDefault();button.onclick=()=>{restore();action();capture();syncTools();};toolbar.append(button);buttons[key]=button;return button;}
 tool('bold','加粗','<svg viewBox="0 0 24 24"><path d="M7 12h6a4 4 0 0 0 0-8H7v16h7a4 4 0 0 0 0-8H7"/></svg>',()=>document.execCommand('bold'));
 tool('highlight','荧光标记','<svg viewBox="0 0 24 24"><path d="m8 14 8-10 4 4-8 10-4-4ZM8 14l-3 5h6M4 21h14"/></svg>',()=>document.execCommand('hiliteColor',false,document.queryCommandValue('backColor')==='rgb(234, 213, 132)'?'transparent':'#ead584'));
 function toggleTodoLine(){
  const selection=getSelection();if(!selection.rangeCount)return;
  const chosen=selection.getRangeAt(0).cloneRange(),wasCollapsed=chosen.collapsed;if(!editor.contains(chosen.commonAncestorContainer))return;
  const point=(node,offset)=>{const range=document.createRange();range.setStart(node,offset);range.collapse(true);return range;};
  const compare=(a,b)=>a.compareBoundaryPoints(Range.START_TO_START,b);
  const start=point(chosen.startContainer,chosen.startOffset),end=point(chosen.endContainer,chosen.endOffset);
  const plans=[];
  for(const host of [...editor.childNodes]){if(host.nodeName==='HR')continue;
   const ranges=[];let part=document.createRange();part.setStart(host,0);
   const finish=(node,offset,nextNode=node,nextOffset=offset)=>{part.setEnd(node,offset);ranges.push(part);part=document.createRange();part.setStart(nextNode,nextOffset);};
   const scan=node=>{
    if(node.nodeType===3){for(let i=0;i<node.length;i++)if(node.data[i]==='\n')finish(node,i,node,i+1);return;}
    for(const child of [...node.childNodes]){const index=[...node.childNodes].indexOf(child);if(child.nodeName==='BR')finish(node,index,node,index+1);else if(['DIV','P'].includes(child.nodeName)){finish(node,index);scan(child);finish(node,index+1);}else scan(child);}
   };
   scan(host);part.setEnd(host,host.nodeType===3?host.length:host.childNodes.length);ranges.push(part);
   const lines=ranges.map(range=>{const a=point(range.startContainer,range.startOffset),b=point(range.endContainer,range.endOffset);const hit=wasCollapsed?range.comparePoint(chosen.startContainer,chosen.startOffset)===0:!!range.toString().trim()&&compare(b,start)>0&&compare(a,end)<0;return {range,hit};});
   if(lines.some(line=>line.hit))plans.push({host,lines});
  }
  const allTodo=plans.length>0&&plans.every(({host,lines})=>lines.filter(line=>line.hit).every(line=>{const node=line.range.startContainer.nodeType===3?line.range.startContainer.parentElement:line.range.startContainer;return !!node.closest?.('.quote-todo')||host.classList?.contains('quote-todo');}));
  let first,last;
  for(const {host,lines} of plans){
   if(lines.length===1&&host.nodeType===1){host.classList.toggle('quote-todo',!allTodo);host.dataset.done='false';first ||= host;last=host;continue;}
   const result=lines.map(({range,hit})=>{const line=document.createElement('div');line.append(range.cloneContents());line.querySelectorAll('.quote-todo').forEach(item=>item.classList.remove('quote-todo'));if(!line.textContent&&!line.querySelector('br'))line.append(document.createElement('br'));if(hit&&!allTodo){line.className='quote-todo';line.dataset.done='false';}if(hit){first ||= line;last=line;}return line;});
   if(host.nodeName==='BLOCKQUOTE'){host.classList.remove('quote-todo');host.replaceChildren(...result);}else host.replaceWith(...result);
  }
  if(first){const range=document.createRange();range.setStart(first,0);range.setEnd(last,last.childNodes.length);if(wasCollapsed)range.collapse(true);selection.removeAllRanges();selection.addRange(range);selected=range.cloneRange();}
 }
 tool('todo','待办','<svg viewBox="0 0 24 24"><rect x="4" y="5" width="16" height="16" rx="3"/><path d="m8 12 3 3 5-6"/></svg>',toggleTodoLine);
 make.className='quote-format-quote';make.title='引用片段';make.setAttribute('aria-label','引用片段');make.innerHTML='<svg viewBox="0 0 24 24"><path d="M4 6h6v7c0 3-2 5-5 5M4 6v7h6M14 6h6v7c0 3-2 5-5 5M14 6v7h6"/></svg>';toolbar.append(make);
 function insertTag(range,text,space=false){
  const label=text.trim().replace(/^#+/,'');if(!label||/[\r\n]/.test(label))return false;
  const origin=range.startContainer.nodeType===3?range.startContainer.parentElement:range.startContainer,draft=origin.closest?.('.quote-tag-draft');
  const tag=document.createElement('span');tag.className='quote-tag';tag.contentEditable='false';tag.textContent=label;stampTag(tag,draft?draft.dataset.createdAt:new Date().toISOString());if(draft?.dataset.status)tag.dataset.status=draft.dataset.status;
  range.deleteContents();range.insertNode(tag);const tail=document.createTextNode(space?' ':'');tag.after(tail);if(draft)draft.replaceWith(...draft.childNodes);
  const caret=document.createRange();caret.setStart(tail,tail.length);caret.collapse(true);const selection=getSelection();selection.removeAllRanges();selection.addRange(caret);selected=caret.cloneRange();return true;
 }
 tool('tag','标签','<svg viewBox="0 0 24 24"><path d="M10 4 8 20M16 4l-2 16M4 9h16M3 15h16"/></svg>',()=>{const selection=getSelection();if(!selection.rangeCount)return;const range=selection.getRangeAt(0);if(!editor.contains(range.commonAncestorContainer))return;if(!range.collapsed){insertTag(range,range.toString());}else document.execCommand('insertText',false,'#');});buttons.tag.removeAttribute('aria-pressed');
 function confirmTypedTag(trailingSpace=false,space=true){
  const selection=getSelection();if(!selection.rangeCount||!selection.isCollapsed)return false;
  const caret=selection.getRangeAt(0),root=currentBlock()||editor;if(!editor.contains(caret.startContainer))return false;
  const before=document.createRange();before.selectNodeContents(root);before.setEnd(caret.startContainer,caret.startOffset);
  const text=before.toString(),match=text.match(trailingSpace?/#([^#\s]+)[ \u00a0\u3000]$/u:/#([^#\s]+)$/u);if(!match)return false;
  let remaining=text.length-match[0].length;const walker=document.createTreeWalker(root,NodeFilter.SHOW_TEXT);let node;
  while(node=walker.nextNode()){if(remaining<=node.length){if(node.parentElement.closest('.quote-tag'))return false;const token=caret.cloneRange();token.setStart(node,remaining);return insertTag(token,match[1],space);}remaining-=node.length;}
  return false;
 }
 function editTag(tag){closeTagPalette();const text=document.createTextNode('#'+tag.textContent),draft=document.createElement('span');draft.className='quote-tag-draft';if(tag.dataset.createdAt)draft.dataset.createdAt=tag.dataset.createdAt;if(tag.dataset.status)draft.dataset.status=tag.dataset.status;draft.append(text);tag.replaceWith(draft);const caret=document.createRange();caret.setStart(text,text.length);caret.collapse(true);const selection=getSelection();selection.removeAllRanges();selection.addRange(caret);selected=caret.cloneRange();capture();syncTools();}
 const tagPalette=document.createElement('div');tagPalette.className='quote-tag-palette';tagPalette.setAttribute('popover','auto');tagPalette.setAttribute('role','group');tagPalette.setAttribute('aria-label','标签状态颜色');el.append(tagPalette);let colorTarget=null;
 const themeButton=document.createElement('button');themeButton.type='button';themeButton.className='note-theme-button';themeButton.setAttribute('aria-label','便签主题颜色');themeButton.title='便签主题颜色';themeButton.innerHTML='<i aria-hidden="true"></i>';el.querySelector('header').insertBefore(themeButton,el.querySelector('header button'));if(tagStates[n.theme]||n.theme==='paper')el.dataset.theme=n.theme;
 function closeTagPalette(){tagPalette.hidePopover();colorTarget=null;}
 for(const [status,label] of [['','默认'],...Object.entries(tagStates),['paper','纸质']]){const button=document.createElement('button');button.type='button';button.dataset.status=status;button.setAttribute('aria-label',label);button.title=label;button.innerHTML='<i aria-hidden="true"></i>';button.onpointerdown=e=>e.preventDefault();button.onclick=()=>{if(colorTarget===el){n.theme=status;el.dataset.theme=status;save();}else if(colorTarget?.isConnected){if(status)colorTarget.dataset.status=status;else delete colorTarget.dataset.status;capture();}closeTagPalette();};tagPalette.append(button);}
 function openColorPalette(tag,target=tag){colorTarget=target;for(const button of tagPalette.children){button.hidden=button.dataset.status==='paper'&&target!==el;button.setAttribute('aria-pressed',String((target===el?n.theme||'':tag.dataset.status||'')===button.dataset.status));}tagPalette.showPopover();const r=tag.getBoundingClientRect(),p=tagPalette.getBoundingClientRect();tagPalette.style.left=Math.max(8,Math.min(r.left,innerWidth-p.width-8))+'px';tagPalette.style.top=(r.bottom+p.height+12<innerHeight?r.bottom+7:Math.max(8,r.top-p.height-7))+'px';}
 themeButton.onclick=e=>{e.stopPropagation();openColorPalette(themeButton,el);};
 editor.addEventListener('click',e=>{const tag=e.target.closest('.quote-tag');if(!tag||e.detail>1)return;e.stopPropagation();openColorPalette(tag);});
 editor.addEventListener('scroll',closeTagPalette,{passive:true});
 editor.addEventListener('dblclick',e=>{const tag=e.target.closest('.quote-tag');if(tag){e.preventDefault();editTag(tag);}});
 editor.addEventListener('input',e=>{if(!composing&&!e.isComposing&&e.inputType==='insertText'&&confirmTypedTag(true)){capture();syncTools();}});
 editor.addEventListener('keydown',e=>{
  if(e.isComposing||composing||e.ctrlKey||e.metaKey||e.altKey)return;
  const selection=getSelection();if(!selection.rangeCount)return;const range=selection.getRangeAt(0),node=range.startContainer,offset=range.startOffset;
  if(e.key==='Backspace'){
   let previous=node.nodeType===3?(offset===0?node.previousSibling:null):node.childNodes[offset-1];
    while(previous?.nodeType===3&&!previous.textContent)previous=previous.previousSibling;
   if(!selection.isCollapsed){const tags=[...editor.querySelectorAll('.quote-tag')].filter(tag=>range.intersectsNode(tag));if(tags.length===1&&selection.toString()===tags[0].textContent)previous=tags[0];}
   if(previous?.nodeType===1&&previous.classList.contains('quote-tag')){e.preventDefault();e.stopImmediatePropagation();editTag(previous);return;}
  }
  if(!selection.isCollapsed)return;
  if((e.key===' '||e.key==='Enter'||e.key==='　')&&confirmTypedTag(false,e.key!=='Enter')){e.preventDefault();e.stopImmediatePropagation();capture();syncTools();}
 });
 tool('divider','分割线','<svg viewBox="0 0 24 24"><path d="M4 12h16"/></svg>',()=>{
  const host=currentBlock(),line=document.createElement('hr'),next=block('');
  if(host)host.after(line,next);else editor.append(line,next);
  const range=document.createRange();range.selectNodeContents(next);range.collapse(true);const selection=getSelection();selection.removeAllRanges();selection.addRange(range);selected=range.cloneRange();next.scrollIntoView({block:'nearest'});
 });buttons.divider.removeAttribute('aria-pressed');
 tool('clear','清除格式','<svg viewBox="0 0 24 24"><path d="M5 5h14M12 5l-3 14m5-5 6 6m0-6-6 6"/></svg>',()=>{document.execCommand('removeFormat');const node=currentBlock();if(node){node.classList.remove('quote-todo');if(node.tagName==='BLOCKQUOTE'){const plain=document.createElement('div');plain.append(...node.childNodes);node.replaceWith(plain);}}});
 editor.addEventListener('click',e=>{const node=e.target.closest('.quote-todo');if(node&&e.clientX<node.getBoundingClientRect().left+parseFloat(getComputedStyle(node).paddingLeft)){node.dataset.done=String(node.dataset.done!=='true');capture();}});
 make.onpointerdown=e=>e.preventDefault();make.onclick=()=>{restore();const existing=currentBlock();if(existing?.tagName==='BLOCKQUOTE'){const plain=document.createElement('div');plain.append(...existing.childNodes);existing.replaceWith(plain);capture();syncTools();return;}if(!selected||!editor.contains(selected.commonAncestorContainer))return;
 const top=node=>{let p=node.nodeType===3?node.parentElement:node;while(p&&p.parentNode!==editor)p=p.parentNode;return p;};
 if(selected.startContainer.nodeType===3&&selected.startOffset===selected.startContainer.length){const edge=top(selected.startContainer);if(edge)selected.setStartAfter(edge);}if(selected.endContainer.nodeType===3&&selected.endOffset===0){const edge=top(selected.endContainer);if(edge)selected.setEndBefore(edge);}const start=top(selected.startContainer),end=top(selected.endContainer);if(start?.tagName==='BLOCKQUOTE'||end?.tagName==='BLOCKQUOTE'){notify('这段文字已在引用块中');return;}
 const text=selected.toString();if(!text.trim())return;
 const before=document.createRange();before.setStart(editor,0);before.setEnd(selected.startContainer,selected.startOffset);
 const after=document.createRange();after.setStart(selected.endContainer,selected.endOffset);after.setEnd(editor,editor.childNodes.length);
 const left=before.cloneContents(),right=after.cloneContents(),quote=block(text,true);quote.replaceChildren(selected.cloneContents());
 editor.replaceChildren(left,quote,right);getSelection().removeAllRanges();make.hidden=false;capture();quote.scrollIntoView({block:'nearest'});
 };
 editor.addEventListener('keydown',e=>{const s=getSelection(),q=s.anchorNode?.parentElement?.closest('blockquote');if(e.key==='Enter'&&(e.ctrlKey||e.metaKey)&&q&&editor.contains(q)){e.preventDefault();const next=block('');q.after(next);const r=document.createRange();r.selectNodeContents(next);r.collapse(true);s.removeAllRanges();s.addRange(r);capture();}if(e.key==='Backspace'&&q&&editor.contains(q)&&!q.innerText.trim()){e.preventDefault();const p=block('');q.replaceWith(p);const r=document.createRange();r.selectNodeContents(p);r.collapse(true);s.removeAllRanges();s.addRange(r);capture();}});
 renderCards();
};
const target=document.createElement('section');target.className='quote-drop-demo';target.innerHTML='<header>输入框 · 试拖到这里</header><textarea aria-label="拖拽接收测试输入框" placeholder="拖入片段，继续编辑…"></textarea>';document.body.append(target);const input=target.querySelector('textarea');input.ondragover=e=>{if(e.dataTransfer.types.includes('text/plain')){e.preventDefault();e.dataTransfer.dropEffect='copy';target.classList.add('drop-ready');}};input.ondragleave=()=>target.classList.remove('drop-ready');input.ondrop=e=>{e.preventDefault();target.classList.remove('drop-ready');input.setRangeText(e.dataTransfer.getData('text/plain'),input.selectionStart,input.selectionEnd,'end');input.focus();};
})();
