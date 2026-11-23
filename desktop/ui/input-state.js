// Shared native-drop hit logic; coordinates are logical pixels after DPI conversion.
(function(root){
 const api={
  createCardDropGate(now=()=>performance.now()){
   let readyAt=Infinity;
   return {
    reset(){readyAt=Infinity},
    expanded(){if(readyAt===Infinity)readyAt=now()+1000},
    ready(){return now()>=readyAt}
   };
  },
  createHoverIntent(open,{schedule=setTimeout,cancel=clearTimeout,delay=180}={}){
   let timer=null,version=0;
   return {
    start(){if(timer!==null)return;const token=++version;timer=schedule(()=>{if(token!==version)return;timer=null;open()},delay)},
    cancel(){version++;if(timer!==null)cancel(timer);timer=null}
   };
  },
  intentionalCardDrop(entry,point){if(!entry)return true;const r=entry.rect;return Math.hypot(point.x-entry.x,point.y-entry.y)>8&&(point.x<r.x||point.x>r.x+r.width||point.y<r.y||point.y>r.y+r.height)},
  acceptsPath(path){return /\.(png|jpe?g|webp)$/i.test(path)}
 };
 if(typeof module!=='undefined')module.exports=api;else root.quickgenInput=api;
})(typeof window==='undefined'?{}:window);
