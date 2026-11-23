
'use strict';
(function(root){
 function create({allowed,importFiles,report,clear=()=>{}}) {
  return async function(event) {
   const path=event.composedPath?.()||[event.target];
   if(event.defaultPrevented||!allowed()||path.some(el=>el?.isContentEditable||el?.matches?.('input,textarea,select,[role="textbox"],[role="dialog"]')))return false;
   event.preventDefault();
   try {
    const files=Array.from(event.clipboardData?.files||[]);
    if(!files.length){report('剪贴板中没有可读取的图片');return false;}
    if(files.some(f=>!/^image\/(png|jpeg|webp)$/.test(f.type)||f.size>20*1024*1024)) {report('仅支持20 MB以内的PNG、JPG或WebP图片');return false;}
    const result=await importFiles(files);
    if(result?.added!==files.length){report('部分或全部图片未能导入，请检查后重试');return false;}
    clear();return true;
   }catch(error){report('粘贴图片失败，请重新复制后重试');return false;}
  };
 }
 const api={create};root.quickgenClipboardIntake=api;if(typeof module!=='undefined')module.exports=api;
})(globalThis);
