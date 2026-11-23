const assert=require('node:assert/strict');
const {clampPicture}=require('../ui/overlay-layout.js');
assert.deepEqual(clampPicture(1700,950,240,170,1280,720),{x:1040,y:550},'shrink must recover offscreen pictures');
assert.deepEqual(clampPicture(100,160,240,170,1920,1080),{x:100,y:160},'a larger screen must not rearrange visible pictures');
assert.deepEqual(clampPicture(-25,-90,240,170,1920,1080),{x:0,y:0},'negative local positions must be visible');
assert.deepEqual(clampPicture(100,160,200,120,200,120),{x:0,y:0},'very small screens must not produce negative coordinates');
console.log('4 overlay layout cases passed');
