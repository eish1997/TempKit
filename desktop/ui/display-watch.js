// Poll monitor metadata, never window size: our own configure calls cannot loop.
(function(root){
 function createDisplayWatch(read,refresh){
  let previous=null,busy=false;
  return async function check(){
   if(busy)return;busy=true;
   try{
    const next=JSON.stringify(await read());
    if(previous===null){previous=next;return}
    if(next!==previous){await refresh();previous=next}
   }finally{busy=false}
  };
 }
 if(typeof module!=='undefined')module.exports=createDisplayWatch;
 else root.createDisplayWatch=createDisplayWatch;
})(typeof window==='undefined'?{}:window);
