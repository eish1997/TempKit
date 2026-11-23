const assert=require('node:assert/strict');
const {place,move,resize,heights}=require('../ui/board-panel-layout.js');
const base={edge:'right',fraction:.5,width:280,height:220,heightIndex:2};
for(const edge of ['top','left','right'])for(const fraction of [0,.5,1])for(const [w,h]of [[1920,1080],[800,600],[100,90]]){
 const p=place({...base,edge,fraction},w,h);for(const r of [p.panel,p.entry]){assert(r.x>=0&&r.y>=0);assert(r.x+r.width<=w&&r.y+r.height<=h)}
}
for(const dir of ['n','s','e','w','ne','nw','se','sw']){
 const s=resize(base,dir,40,30,1200,800);
 assert.equal(s.width,dir.includes('e')?320:dir.includes('w')?240:280);
 assert.equal(s.height,dir.includes('s')?250:dir.includes('n')?190:220);
 assert.equal(place(s,1200,800).panel.x+s.width,1200,'right edge stays attached');
}
assert.equal(move(base,400,5,1200,800).edge,'top');
assert.equal(move(base,5,400,1200,800).edge,'left');
assert.equal(move(base,1195,400,1200,800).edge,'right');
assert.equal(resize(base,'w',1000,0,1200,800).width,120);
const old=place(base,1200,800).panel,newRect=place(resize(base,'n',0,40,1200,800),1200,800).panel;
assert.equal(old.y+old.height,newRect.y+newRect.height,'opposite vertical edge remains fixed');
assert.deepEqual(heights,[32,56,80,112]);
assert.deepEqual(place(JSON.parse(JSON.stringify(base)),1200,800).state,base,'saved legacy fields round trip');
console.log('Panel edge, eight resize directions, minimum width, bounds and legacy layout cases passed');
