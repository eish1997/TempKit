(function(root){'use strict';
const themes=['default','yellow','green','blue','pink','purple','paper'];
function apply(target,theme){target.dataset.noteTheme=themes.includes(theme)?theme:'default';target.classList.add('note-appearance');}
// Overview is a bounded, non-editable projection; it never creates editor views.
function preview(target,note){target.replaceChildren();apply(target,note.theme);const content=document.createElement('div');content.className='note-document-summary';target.append(content);let chars=1800,nodes=96;
 const append=(parent,n)=>{if(chars<=0||nodes--<=0)return;let el;
  if(n.type==='text'){const text=(n.text||'').slice(0,chars);chars-=text.length;el=document.createTextNode(text);for(const mark of n.marks||[]){if(!['strong','highlight'].includes(mark.type))continue;const outer=document.createElement(mark.type==='strong'?'strong':'mark');outer.append(el);el=outer;}}
  else if(n.type==='tag'){el=document.createElement('span');el.className='note-tag';el.dataset.color=themes.includes(n.attrs?.color)?n.attrs.color:'default';el.textContent=(n.attrs?.label||'').slice(0,chars);chars-=el.textContent.length;}
  else if(n.type==='hard_break')el=document.createElement('br');
  else if(['doc','paragraph','blockquote','horizontal_rule'].includes(n.type)){el=document.createElement(({doc:'div',paragraph:'p',blockquote:'blockquote',horizontal_rule:'hr'})[n.type]);if(n.type==='paragraph'){el.dataset.todo=String(!!n.attrs?.todo);el.dataset.done=String(!!n.attrs?.done);}for(const child of n.content||[]){append(el,child);if(chars<=0||nodes<=0)break;}}
  if(el)parent.append(el);
 };
 if(note.document?.version===1)append(content,note.document.doc);else content.textContent=(note.text||'').slice(0,1800);
}
const api={themes,apply,preview};if(typeof module==='object')module.exports=api;else root.quickgenNoteAppearance=api;
})(typeof window==='object'?window:globalThis);
