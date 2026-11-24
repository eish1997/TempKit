import {TextSelection} from 'prosemirror-state';
import {liftTarget} from 'prosemirror-transform';
import {schema,selectedParagraphs,projection} from './schema.mjs';

// Work on logical paragraphs, including only the selected part of a quote.
function groups(state){const result=[];for(const item of selectedParagraphs(state)){const $p=state.doc.resolve(item.pos),parent=$p.parent,last=result.at(-1);if(last&&last.parent===parent&&last.to===item.pos){last.to+=item.node.nodeSize;}else result.push({parent,from:item.pos,to:item.pos+item.node.nodeSize});}return result;}
export function toggleQuote(state,dispatch){
 const selected=groups(state);if(!selected.length)return false;
 const remove=selected.every(x=>x.parent.type===schema.nodes.blockquote);let tr=state.tr;
 for(const group of selected.reverse()){
  const range=tr.doc.resolve(group.from+1).blockRange(tr.doc.resolve(group.to-1));if(!range)continue;
  if(remove){const target=liftTarget(range);if(target!=null)tr.lift(range,target);}
  else if(group.parent.type===schema.nodes.doc)tr.wrap(range,[{type:schema.nodes.blockquote,attrs:{id:crypto.randomUUID()}}]);
 }
 if(!tr.docChanged)return false;dispatch?.(tr.scrollIntoView());return true;
}
export function insertRule(state,dispatch){
 let tr=state.tr.replaceSelectionWith(schema.nodes.horizontal_rule.create());
 const after=tr.selection.$to; if(after.parent===tr.doc&&after.pos===tr.doc.content.size)tr.insert(after.pos,schema.nodes.paragraph.create());
 // Always leave a text insertion point after the separator, including at EOF.
 const selected=TextSelection.findFrom(tr.doc.resolve(Math.min(tr.selection.to,tr.doc.content.size)),1,true);
 if(selected)tr.setSelection(selected);dispatch?.(tr.scrollIntoView());return true;
}
export function clearFormat(state,dispatch){
 let tr=state.tr;let {from,to}=state.selection;
 if(state.selection.empty){const p=selectedParagraphs(state)[0];if(!p)return false;from=p.pos+1;to=p.pos+p.node.nodeSize-1;}
 tr.removeMark(from,to);
 const tags=[];state.doc.nodesBetween(from,to,(node,pos)=>{if(node.type===schema.nodes.tag)tags.push({node,pos});});
 for(const {node,pos} of tags.reverse())tr.replaceWith(pos,pos+node.nodeSize,schema.text(node.attrs.label));
 let working=state.apply(tr);const selected=selectedParagraphs(working);
 for(const {node,pos}of selected)tr.setNodeMarkup(pos,null,{...node.attrs,todo:false,done:false});
 working=state.apply(tr);
 for(const group of groups(working).reverse())if(group.parent.type===schema.nodes.blockquote){const range=tr.doc.resolve(group.from+1).blockRange(tr.doc.resolve(group.to-1));const target=range&&liftTarget(range);if(target!=null)tr.lift(range,target);}
 tr.setStoredMarks([]);dispatch?.(tr.scrollIntoView());return true;
}
export function createTag(state,dispatch){
 const {from,to,empty,$from,$to}=state.selection;if(empty){dispatch?.(state.tr.insertText('#'));return true;}if(!$from.sameParent($to))return false;
 const label=state.doc.textBetween(from,to,'',leaf=>leaf.type===schema.nodes.tag?leaf.attrs.label:'').trim();
 if(!label||/[\r\n\t]/u.test(label)||new TextEncoder().encode(label).length>4096)return false;
 dispatch?.(state.tr.replaceSelectionWith(schema.nodes.tag.create({id:crypto.randomUUID(),label,createdAt:0,color:'default'})).scrollIntoView());return true;
}
export function convertHash(state,dispatch){
 if(!state.selection.empty)return false;const {$from}=state.selection;
 const prefix=$from.parent.textBetween(0,$from.parentOffset,'','\ufffc');
 const match=/(?:^|\s)#([^\s#\ufffc]+)$/u.exec(prefix);if(!match)return false;
 const label=match[1];if(new TextEncoder().encode(label).length>4096)return false;
 const start=$from.pos-label.length-1;
 dispatch?.(state.tr.replaceWith(start,$from.pos,schema.nodes.tag.create({id:crypto.randomUUID(),label,createdAt:0,color:'default'})));return true;
}
export function plainSlice(slice){return projection(schema.nodes.doc.create(null,slice.content));}
