// Delayed host responses exercise actual render functions, not native rendering.
const {test}=require('node:test'),assert=require('node:assert/strict');
const vm=require('node:vm'),fs=require('node:fs'),path=require('node:path');
function fixture(kind){
 const pending=[];
 const element=()=>({children:[],dataset:{},append(...children){this.children.push(...children)},prepend(child){this.children.unshift(child)},replaceChildren(){this.children=[]}});
 const root=element();
 const context=vm.createContext({document:{createElement:element},$:()=>root,content:root,abilities:[],
  quickgenIcons:{check:"",plus:""},presets:[{id:'one',name:'One',prompt:'Prompt'}],save:async()=>{},fail(){},
  invoke:()=>new Promise((resolve,reject)=>pending.push({resolve,reject}))});
 const app=fs.readFileSync(path.join(__dirname,kind==='main'?'../ui/app.js':'../ui/manager.js'),'utf8');
 const marker=kind==='main'?'async function renderAbilities()':'async function library()';
 const end=kind==='main'?'(async()=>{try{const cfg=':'const mode=';
 vm.runInContext('let abilityRenderSequence=0,libraryRenderSequence=0;'+app.slice(app.indexOf(marker),app.indexOf(end,app.indexOf(marker))),context);
 return {root,pending,refresh:()=>vm.runInContext(kind==='main'?'renderAbilities()':'library()',context)};
}
for(const kind of ['main','library'])test(`${kind}: older response cannot remove a newly added card`,async()=>{
 const f=fixture(kind),old=f.refresh(),latest=f.refresh();
 f.pending[1].resolve({abilities:[{id:'one',name:'One'}]});await latest;
 f.pending[0].resolve({abilities:[]});await old;
 if(kind==='main')assert.equal(f.root.children[0]?.dataset.ability,'one');
 else assert.equal(f.root.children[0].children[1].disabled,true);
});
test('failed refresh leaves displayed cards intact and a subsequent refresh recovers',async()=>{
 const f=fixture('main');let work=f.refresh();f.pending[0].resolve({abilities:[{id:'one',name:'One'}]});await work;
 work=f.refresh();f.pending[1].reject(Error('read failed'));await assert.rejects(work,/read failed/);
 assert.equal(f.root.children[0].dataset.ability,'one');
 work=f.refresh();f.pending[2].resolve({abilities:[{id:'two',name:'Two'}]});await work;
 assert.equal(f.root.children[0].dataset.ability,'two');
});
