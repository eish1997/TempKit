(function(root){
'use strict';
class Hints{
 constructor(changed=()=>{}){this.rows=new Map();this.changed=changed;this.clock=0;}
 set(id,row){const previous=this.rows.get(id);this.rows.set(id,{...row,id,order:previous?.order??++this.clock});this.changed(this.current());}
 clear(id){if(this.rows.delete(id))this.changed(this.current());}
 current(){return [...this.rows.values()].sort((a,b)=>b.priority-a.priority||b.order-a.order)[0]||null;}
}
root.quickgenHints={Hints};if(typeof module==='object')module.exports={Hints};
})(typeof window==='object'?window:globalThis);
