const assert=require('node:assert/strict'),{Hints}=require('../ui/panel-hints.js');
const h=new Hints();h.set('tasks',{priority:10});h.set('empty',{priority:20});
for(let i=0;i<10;i++)h.set('tasks',{priority:10,text:String(i)});
assert.equal(h.current().id,'empty');h.set('undo:A',{priority:30});h.set('error:A',{priority:40});h.set('error:B',{priority:40});
h.clear('error:A');assert.equal(h.current().id,'error:B');h.clear('error:B');assert.equal(h.current().id,'undo:A');h.clear('undo:A');assert.equal(h.current().id,'empty');h.clear('empty');assert.equal(h.current().id,'tasks');
console.log('Hint source isolation, error/undo/guide/task priority and polling stability passed.');

const {reserveBelow}=require('../ui/board-panel-layout.js');
const panel={x:100,y:650,width:280,height:150};const fitted=reserveBelow(panel,800,50);
assert.equal(fitted.y+fitted.height+50,800);assert.equal(panel.y,650);assert.equal(reserveBelow(panel,100,40).height,60);
console.log('Panel and hint group stays in viewport without mutating saved geometry.');
