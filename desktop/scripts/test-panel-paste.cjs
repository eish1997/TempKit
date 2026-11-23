const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict');
const source=fs.readFileSync('desktop/ui/board.js','utf8');
const registration=source.split('\n').find(line=>line.startsWith("document.addEventListener('paste',"));
let options;
const context={document:{hasFocus:()=>true,addEventListener(){}},quickgenClipboardIntake:{create(value){options=value;}},open:false,passthrough:false,notesMode:{active:false},noteBarrier:false,quitting:false,managerOpening:false,storageReady:true,importFiles(){},hints:{clear(){},set(){}}};
vm.runInNewContext(registration,context);
assert.equal(options.allowed(),true,'focused main panel must accept paste with canvas closed');
for(const guard of ['passthrough','noteBarrier','quitting','managerOpening']){context[guard]=true;assert.equal(options.allowed(),false,guard);context[guard]=false;}
context.notesMode.active=true;assert.equal(options.allowed(),false);context.notesMode.active=false;
context.storageReady=false;assert.equal(options.allowed(),false);
console.log('Actual paste registration: collapsed main panel accepted, unsafe modes rejected');
