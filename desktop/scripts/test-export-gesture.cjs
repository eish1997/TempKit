const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict');
const source=fs.readFileSync('desktop/ui/board.js','utf8').replace(/\r\n/g,'\n');const head=source.slice(source.indexOf('function startDrag('),source.indexOf('\n\n',source.indexOf('function startDrag(')));
let exportCalls=0;const context={dragOut:()=>{exportCalls++},trackBoardWork(){}};vm.createContext(context);vm.runInContext(head+'\nreturn "internal";}',context);
function event(modifiers={}){return {button:0,target:{closest:()=>false},preventDefault(){},stopPropagation(){},...modifiers};}
context.startDrag(event({ctrlKey:true}),{},{});assert.equal(exportCalls,1);
for(const mods of [{},{altKey:true},{ctrlKey:true,altKey:true},{ctrlKey:true,shiftKey:true},{ctrlKey:true,button:2}]){assert.equal(context.startDrag(event(mods),{},{}),'internal');}
assert.equal(context.startDrag(event({ctrlKey:true}),{},{},'stack'),'internal');assert.equal(exportCalls,1);
console.log('Actual drag gesture: Ctrl directly exportCalls; normal/Alt/Shift/internal stack paths preserved');
