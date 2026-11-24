import {EditorState,TextSelection,Plugin} from 'prosemirror-state';
import {EditorView} from 'prosemirror-view';
import {DOMParser,Fragment,Slice} from 'prosemirror-model';
import {baseKeymap,toggleMark,chainCommands,exitCode} from 'prosemirror-commands';
import {history,undo,redo} from 'prosemirror-history';
import {keymap} from 'prosemirror-keymap';
import {schema,fromText,projection,toggleTodo,unwrapTagBefore,unwrapTagAfter,selectedParagraphs,colors} from './schema.mjs';
import {toggleQuote,insertRule,clearFormat,createTag,convertHash} from './commands.mjs';
import {mountQuoteTabs} from './quote-tabs.mjs';

// A paste is a new instance: retain safe structure and user text, never identity.
function copied(fragment){return Fragment.fromArray(fragment.content.map(node=>{
 const attrs=node.type===schema.nodes.tag?{...node.attrs,id:crypto.randomUUID(),createdAt:0}:node.type===schema.nodes.blockquote?{id:crypto.randomUUID()}:node.attrs;
 return node.isText?node:node.type.create(attrs,node.isLeaf?null:copied(node.content),node.marks);
}));}
function safeHTML(html){const template=document.createElement('template');template.innerHTML=html;
 for(const el of template.content.querySelectorAll('script,style,iframe,object,embed,img,video,audio,source,link,meta,svg,math'))el.remove();
 for(const el of template.content.querySelectorAll('*')){const tag=el.tagName==='SPAN'&&el.hasAttribute('data-note-tag'),color=el.dataset.color;for(const a of [...el.attributes])el.removeAttribute(a.name);if(tag&&el.textContent.trim()&&new TextEncoder().encode(el.textContent.trim()).length<=4096&&!/[\r\n\t]/u.test(el.textContent)){el.dataset.noteTag='true';el.dataset.color=colors.includes(color)?color:'default';}}
 return template.innerHTML;
}
function normalizeIds(){return new Plugin({appendTransaction(transactions,old,state){if(!transactions.some(t=>t.docChanged))return null;const ids=new Set(),tr=state.tr;state.doc.descendants((n,pos)=>{if(!['tag','blockquote'].includes(n.type.name))return;let id=n.attrs.id;if(!id||ids.has(id)){id=crypto.randomUUID();tr.setNodeMarkup(pos,null,{...n.attrs,id,...(n.type===schema.nodes.tag?{createdAt:0}:{})});}ids.add(id);});return tr.docChanged?tr:null;}});}

export function createEditor(element){
 let disabled=true,readOnly=false,unsupported='',composition=false,palette=null,view,finishTagEdit=null,tabs;const toolbarButtons=[];
 const syncTools=()=>{if(!view)return;const state=view.state,{from,to,empty,$from}=state.selection,paragraphs=selectedParagraphs(state),marks=state.storedMarks||$from.marks();for(const [name,button]of toolbarButtons){button.disabled=disabled||readOnly||!!unsupported||composition;let active=null;if(name==='bold'||name==='highlighter'){const mark=schema.marks[name==='bold'?'strong':'highlight'];active=empty?!!mark.isInSet(marks):state.doc.rangeHasMark(from,to,mark);}if(name==='list-todo')active=paragraphs.length>0&&paragraphs.every(p=>p.node.attrs.todo);if(name==='quote')active=paragraphs.length>0&&paragraphs.every(p=>state.doc.resolve(p.pos).parent.type===schema.nodes.blockquote);if(active!==null)button.setAttribute('aria-pressed',String(active));}};
 const allowed=()=>!disabled&&!readOnly&&!unsupported&&!composition&&!view.composing;
 const notify=()=>element.dispatchEvent(new Event('input',{bubbles:true}));
 const command=fn=>{if(!allowed())return false;try{finishTagEdit?.();}catch(error){element.dispatchEvent(new CustomEvent('note-interaction-error',{bubbles:true,detail:String(error)}));return false;}const result=fn(view.state,view.dispatch,view);view.focus();return result;};
 const closePalette=()=>{palette?.remove();palette=null;};
 const paragraphView=(node,editor,getPos)=>{
  const dom=document.createElement('p'),button=document.createElement('button'),contentDOM=document.createElement('span');contentDOM.className='paragraph-text';button.type='button';button.contentEditable='false';button.className='todo-toggle';button.setAttribute('aria-label','切换待办完成状态');dom.append(button,contentDOM);
  const update=n=>{if(n.type!==schema.nodes.paragraph)return false;node=n;dom.dataset.todo=String(n.attrs.todo);dom.dataset.done=String(n.attrs.done);button.hidden=!n.attrs.todo;button.setAttribute('aria-pressed',String(n.attrs.done));const glyph=n.attrs.done?'✓':'';if(button.textContent!==glyph)button.textContent=glyph;return true;};update(node);
  button.onpointerdown=e=>e.preventDefault();button.onclick=()=>{if(!allowed())return;const pos=getPos();editor.dispatch(editor.state.tr.setNodeMarkup(pos,null,{...node.attrs,done:!node.attrs.done}));editor.focus();};
  return {dom,contentDOM,update,stopEvent:e=>button.contains(e.target),ignoreMutation:m=>m.type==='attributes'&&m.target!==contentDOM};
 };
 const tagView=(node,editor,getPos)=>{
  const dom=document.createElement('span');dom.className='note-tag';dom.contentEditable='false';let editing=false;
  const update=n=>{if(n.type!==schema.nodes.tag||n.attrs.id!==node.attrs.id)return false;node=n;if(!editing&&dom.textContent!==n.attrs.label)dom.textContent=n.attrs.label;dom.dataset.color=n.attrs.color;dom.dataset.tagId=n.attrs.id;dom.title=n.attrs.createdAt?new Date(n.attrs.createdAt).toLocaleString():'';return true;};update(node);
  dom.onclick=e=>{if(!allowed())return;closePalette();e.preventDefault();palette=document.createElement('div');palette.className='note-palette tag-palette';palette.setAttribute('role','group');palette.setAttribute('aria-label','标签颜色');
   for(const color of colors){const b=document.createElement('button');b.type='button';b.dataset.color=color;b.setAttribute('aria-label',color);b.setAttribute('aria-pressed',String(color===node.attrs.color));b.onpointerdown=e=>e.preventDefault();b.onclick=()=>{if(!allowed())return;const pos=getPos();editor.dispatch(editor.state.tr.setNodeMarkup(pos,null,{...node.attrs,color}));closePalette();editor.focus();};palette.append(b);}document.body.append(palette);const rect=dom.getBoundingClientRect(),width=palette.getBoundingClientRect().width;palette.style.left=Math.max(40,Math.min(rect.left,innerWidth-width-8))+'px';palette.style.top=Math.max(38,Math.min(rect.bottom+5,innerHeight-45))+'px';
  };
  dom.ondblclick=e=>{if(!allowed()||editing)return;e.preventDefault();closePalette();const input=document.createElement('input');input.value=node.attrs.label;input.className='tag-rename';input.setAttribute('aria-label','修改标签文字');editing=true;dom.replaceChildren(input);input.focus();input.select();let finished=false;
   const finish=(save,barrier=false)=>{if(finished)return;if(input.isComposing){if(barrier)throw Error('标签输入法组合尚未结束');return;}const label=input.value.trim();if(save&&(!label||/[\r\n\t]/u.test(label)||new TextEncoder().encode(label).length>4096)){if(barrier)throw Error('请完成标签文字，或按 Esc 取消');return;}if(save&&!allowed()&&!barrier)return;finished=true;editing=false;finishTagEdit=null;if(save)editor.dispatch(editor.state.tr.setNodeMarkup(getPos(),null,{...node.attrs,label}));update(node);if(!barrier)editor.focus();};finishTagEdit=()=>finish(true,true);
   input.addEventListener('compositionstart',()=>input.isComposing=true);input.addEventListener('compositionend',()=>input.isComposing=false);
   input.onkeydown=e=>{e.stopPropagation();if(e.isComposing||input.isComposing)return;if(e.key==='Enter'){e.preventDefault();finish(true);}if(e.key==='Escape'){e.preventDefault();finish(false);}};input.onblur=()=>finish(true);
  };
  return {dom,update,stopEvent:()=>true,ignoreMutation:()=>true};
 };
 const guardedKeys={};for(const [key,fn]of Object.entries({...baseKeymap,'Mod-z':undo,'Mod-y':redo,'Shift-Mod-z':redo,'Mod-b':toggleMark(schema.marks.strong),'Backspace':chainCommands(unwrapTagBefore,baseKeymap.Backspace),'Delete':chainCommands(unwrapTagAfter,baseKeymap.Delete),'Enter':chainCommands(convertHash,baseKeymap.Enter),'Space':convertHash}))guardedKeys[key]=(s,d,v)=>allowed()?fn(s,d,v):false;
 view=new EditorView(element,{state:EditorState.create({schema,doc:fromText(''),plugins:[history({depth:100}),keymap(guardedKeys),normalizeIds()]}),editable:()=>!disabled&&!readOnly&&!unsupported,attributes:{role:'textbox','aria-label':'便签内容','aria-multiline':'true',spellcheck:'false'},nodeViews:{paragraph:paragraphView,tag:tagView},
  dispatchTransaction(tr){view.updateState(view.state.apply(tr));syncTools();if(tr.docChanged){tabs?.render();notify();}},
  transformPastedHTML:safeHTML,transformPasted:s=>new Slice(copied(s.content),s.openStart,s.openEnd),
  clipboardTextSerializer:slice=>{const lines=[];slice.content.descendants(n=>{if(n.isText)lines.push(n.text);else if(n.type===schema.nodes.tag)lines.push(n.attrs.label);else if(n.type===schema.nodes.hard_break)lines.push('\n');});return slice.content.textBetween(0,slice.content.size,'\n',n=>n.type===schema.nodes.tag?n.attrs.label:n.type===schema.nodes.hard_break?'\n':'');},
  handleDOMEvents:{compositionstart(){composition=true;syncTools();return false;},compositionend(){composition=false;syncTools();return false;},blur(){closePalette();return false;}}
 });
 tabs=mountQuoteTabs(view,element,document.getElementById('quote-tabs'));
 const setState=doc=>{view.updateState(EditorState.create({schema,doc,plugins:view.state.plugins}));tabs.render();syncTools();};
 const api={view,command,
  load(record){unsupported='';try{if(record.document?.version!==1)throw Error('未知便签格式，正文只读保留');const doc=schema.nodeFromJSON(record.document.doc);doc.check();setState(doc);}catch(error){unsupported='此便签格式暂不支持，已保留原数据。';setState(fromText(record.text||''));}view.setProps({});return unsupported;},
  snapshot(){return {version:1,doc:view.state.doc.toJSON()};},
  prepareSave(){finishTagEdit?.();},
  key(document){return JSON.stringify({version:1,doc:schema.nodeFromJSON(document.doc).toJSON()});},
  // Apply only host-owned metadata to the CURRENT draft, never replace newer typing.
  acknowledge(document){const times=new Map();const scan=n=>{if(n.type==='tag')times.set(n.attrs.id,n.attrs.createdAt);n.content?.forEach(scan);};scan(document.doc);const tr=view.state.tr;view.state.doc.descendants((n,pos)=>{if(n.type===schema.nodes.tag&&times.has(n.attrs.id)&&n.attrs.createdAt!==times.get(n.attrs.id))tr.setNodeMarkup(pos,null,{...n.attrs,createdAt:times.get(n.attrs.id)});});if(tr.docChanged)view.updateState(view.state.apply(tr.setMeta('addToHistory',false)));},
  get error(){return unsupported;},get disabled(){return disabled;},set disabled(v){disabled=!!v;view.setProps({});syncTools();},get readOnly(){return readOnly||!!unsupported;},set readOnly(v){readOnly=!!v;view.setProps({});syncTools();},
  get value(){return projection(view.state.doc);},set value(text){setState(fromText(text));},
  focus(){view.focus();},addEventListener(...args){element.addEventListener(...args);},destroy(){closePalette();tabs.destroy();view.destroy();}
 };
 const toolbar=document.getElementById('note-tools');
 const actions=[['bold','加粗',toggleMark(schema.marks.strong)],['highlighter','荧光标记',toggleMark(schema.marks.highlight)],['list-todo','待办',toggleTodo],['quote','引用',toggleQuote],['hash','标签',createTag],['minus','分割线',insertRule],['remove-formatting','清除格式',clearFormat]];
 if(toolbar)for(const [icon,title,fn]of actions){const b=document.createElement('button');b.type='button';b.title=title;b.setAttribute('aria-label',title);b.dataset.command=icon;b.innerHTML=window.quickgenIcons?.[icon]||title;b.onpointerdown=e=>e.preventDefault();b.onclick=()=>command(fn);toolbar.append(b);toolbarButtons.push([icon,b]);}syncTools();
 element.noteEditor=api;return api;
}
window.createQuickgenNoteEditor=createEditor;
