const fs=require('node:fs'),path=require('node:path'),{JSDOM}=require('jsdom');
const ui=path.join(__dirname,'../ui');
function documentOf(text){return {version:1,doc:{type:'doc',content:text.split('\n').map(line=>({type:'paragraph',...(line?{content:[{type:'text',text:line}]}:{})}))}};}
function record(text='',extra={}){return {id:'note-test',text,document:documentOf(text),text_revision:0,theme:'default',theme_revision:0,resident:true,always_on_top:false,...extra};}
function harness(invoke){
 const dom=new JSDOM(fs.readFileSync(path.join(ui,'note.html'),'utf8'),{url:'https://quickgen.test/note.html?id=note-test',runScripts:'outside-only',pretendToBeVisual:true});
 const w=dom.window,events={};w.TextEncoder=TextEncoder;w.TextDecoder=TextDecoder;w.ClipboardEvent=w.Event;w.scrollBy=()=>{};
 w.Range.prototype.getClientRects=()=>[];w.Range.prototype.getBoundingClientRect=()=>({top:0,left:0,right:10,bottom:10,width:10,height:10});
 w.__TAURI__={core:{invoke,Channel:class{}},event:{listen:async(k,v,options)=>{if(options?.target?.label!=='note-test')throw Error('wrong event target');events[k]=v;}}};
 for(const f of ['icons.js','overlay-icons.js','note-icons.js','note-appearance.js','note-editor.bundle.js'])w.eval(fs.readFileSync(path.join(ui,f),'utf8'));
 const start=()=>w.eval(fs.readFileSync(path.join(ui,'note.js'),'utf8'));
 const elements=new Proxy({},{get:(_,id)=>id==='text'?w.document.getElementById('text').noteEditor:w.document.getElementById(id)});
 return {dom,w,events,elements,start,close(){elements.text?.destroy();w.close();},composition(type){elements.text.view.dom.dispatchEvent(new w.CompositionEvent(type,{bubbles:true}));}};
}
module.exports={harness,record,documentOf};
