// Browser-only visual fixture. Never connects to the host or user storage.
const fs=require('node:fs'),path=require('node:path');
const root=path.resolve(__dirname,'..'),output=path.join(root,'evidence/notes-rich-migration');
const p=(text,attrs={})=>({type:'paragraph',attrs,content:[{type:'text',text}]});
const doc={version:1,doc:{type:'doc',content:[p('下一轮材质调整'),{type:'blockquote',attrs:{id:'quote-light'},content:[p('柔和的侧面自然光，保留木材的细微纹理。'),p('阴影干净，不要过度锐化。',{todo:true,done:true})]},p('先比较两版光照，再决定用哪一版。'),{type:'paragraph',content:[{type:'tag',attrs:{id:'tag-material',label:'浅橡木',createdAt:1795450000000,color:'green'}},{type:'text',text:' 与米白色织物，减少反光。',marks:[{type:'highlight'}]}]},{type:'horizontal_rule'},p('完成之后再检查画面边缘。',{todo:true,done:false})]}};
const note={id:'preview',document:doc,text:'下一轮材质调整',theme:process.argv[2]||'default',theme_revision:0,text_revision:0,resident:true,always_on_top:false};
const stub=`<script>window.__TAURI__={core:{invoke:async(name,args)=>{if(name==='note_get')return ${JSON.stringify(note)};if(name==='load_config')return {theme:${JSON.stringify(process.argv[3]||'light')}};if(name==='note_patch')return {...${JSON.stringify(note)},document:args.patch.document};}},event:{listen:async()=>()=>{}}};</script>`;
const html=fs.readFileSync(path.join(root,'ui/note.html'),'utf8').replace(/(src|href)="([^":]+)"/g,'$1="../../ui/$2"').replace('<script src="../../ui/icons.js">',stub+'<script src="../../ui/icons.js">');
fs.writeFileSync(path.join(output,'rich-note-preview.html'),html);
