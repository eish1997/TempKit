import {Schema} from 'prosemirror-model';
import {NodeSelection,TextSelection} from 'prosemirror-state';
export const colors=['default','yellow','green','blue','pink','purple','paper'];
export const schema=new Schema({nodes:{
 doc:{content:'block+'},
 paragraph:{group:'block',content:'inline*',attrs:{todo:{default:false},done:{default:false}},parseDOM:[{tag:'p'},{tag:'div'}],toDOM:n=>['p',{'data-todo':n.attrs.todo?'true':null,'data-done':n.attrs.done?'true':null},0]},
 blockquote:{group:'block',content:'paragraph+',attrs:{id:{default:null}},defining:true,parseDOM:[{tag:'blockquote',getAttrs:()=>({id:crypto.randomUUID()})}],toDOM:n=>['blockquote',{'data-quote-id':n.attrs.id},0]},
 horizontal_rule:{group:'block',parseDOM:[{tag:'hr'}],toDOM:()=>['hr']},
 text:{group:'inline'},
 hard_break:{inline:true,group:'inline',selectable:false,parseDOM:[{tag:'br'}],toDOM:()=>['br']},
 tag:{inline:true,group:'inline',atom:true,selectable:true,attrs:{id:{},label:{},createdAt:{},color:{default:'default'}},parseDOM:[{tag:'span[data-note-tag]',getAttrs:el=>({id:crypto.randomUUID(),label:el.textContent.trim(),createdAt:0,color:colors.includes(el.dataset.color)?el.dataset.color:'default'})}],toDOM:n=>['span',{'class':'note-tag','data-note-tag':'true','data-color':n.attrs.color,contenteditable:'false'},n.attrs.label]}
},marks:{strong:{parseDOM:[{tag:'strong'},{tag:'b'}],toDOM:()=>['strong',0]},highlight:{parseDOM:[{tag:'mark'}],toDOM:()=>['mark',0]}}});
export function fromText(text){return schema.node('doc',null,text.split('\n').map(line=>schema.node('paragraph',null,line?[schema.text(line)]:[])));}
export function projection(doc){const lines=[];doc.descendants(n=>{if(n.type.name==='paragraph'){lines.push(n.textBetween(0,n.content.size,'',leaf=>leaf.type.name==='tag'?leaf.attrs.label:leaf.type.name==='hard_break'?'\n':''));return false;}if(n.type.name==='horizontal_rule'){lines.push('');return false;}});return lines.join('\n');}
// Selection end at a following paragraph's start does not include that paragraph.
export function selectedParagraphs(state){const result=[];const {from,to,empty,$from}=state.selection;if(empty){for(let d=$from.depth;d>0;d--)if($from.node(d).type.name==='paragraph'){result.push({node:$from.node(d),pos:$from.before(d)});break;}return result;}
 state.doc.nodesBetween(from,to,(node,pos)=>{if(node.type.name==='paragraph'&&pos+1<to&&pos+node.nodeSize-1>=from){result.push({node,pos});return false;}});return result;}
export function toggleTodo(state,dispatch){const selected=selectedParagraphs(state);if(!selected.length)return false;const enabled=!selected.every(x=>x.node.attrs.todo);let tr=state.tr;for(const {node,pos}of selected)tr=tr.setNodeMarkup(pos,null,{...node.attrs,todo:enabled,done:false});dispatch?.(tr);return true;}
function unwrapTag(state,dispatch,forward){const selection=state.selection,{$from,empty}=selection;let node,start;if(selection instanceof NodeSelection){node=selection.node;start=selection.from;}else if(empty){node=forward?$from.nodeAfter:$from.nodeBefore;start=forward?$from.pos:$from.pos-(node?.nodeSize||0);}else return false;if(node?.type.name!=='tag')return false;const text='#'+node.attrs.label,tr=state.tr.replaceWith(start,start+node.nodeSize,schema.text(text));tr.setSelection(TextSelection.create(tr.doc,forward?start:start+text.length));dispatch?.(tr);return true;}
export function unwrapTagBefore(state,dispatch){return unwrapTag(state,dispatch,false);}
export function unwrapTagAfter(state,dispatch){return unwrapTag(state,dispatch,true);}
export function runOutsideComposition(view,command){if(view.composing)return false;return command(view.state,view.dispatch,view);}
