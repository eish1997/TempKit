(function(root){
  function clampPicture(x,y,width,height,viewportWidth,viewportHeight){
    return {x:Math.max(0,Math.min(x,Math.max(0,viewportWidth-width))),y:Math.max(0,Math.min(y,Math.max(0,viewportHeight-height)))};
  }
  if(typeof module==='object'&&module.exports)module.exports={clampPicture};
  else root.overlayLayout={clampPicture};
})(globalThis);
