'use strict';
window.quickgenImageStorage={async mount(content,invoke,fail){
 content.innerHTML='<p id="storage-usage" role="status">正在读取暂存空间…</p><p id="storage-counts"></p><p>待保存结果受保护。清理只移除已有完整副本的重复暂存。遗留文件先备份并校验，再移出暂存区；恢复副本仍占磁盘空间。</p><div class="actions"><button id="storage-refresh" type="button">刷新</button><button id="storage-clean" type="button">清理已保存副本</button><button id="storage-archive" type="button">备份并移出遗留文件</button></div><p id="storage-drag-usage" role="status"></p><p>拖出副本供目标软件读取。请确认复制已完成后再清理；每次最多核验 32 MiB / 64 个副本。占用中、已修改及未完成的文件会保留，画板原图不受影响。</p><div class="actions"><button id="storage-drag-clean" type="button">清理拖出副本</button><button id="storage-drag-archive" type="button">备份并移出拖出文件</button><button id="storage-drag-open" type="button">查看拖出目录</button></div><div id="storage-confirm" role="group" aria-labelledby="storage-confirm-text" hidden><p id="storage-confirm-text"></p><div class="actions"><button id="storage-confirm-cancel" type="button">取消</button><button id="storage-confirm-accept" type="button">确认</button></div></div><p id="storage-result" role="status"></p><div class="actions"><button id="storage-open" type="button" hidden>打开恢复副本</button><button id="storage-back" type="button">返回连接</button></div>';
 const get=id=>content.querySelector('#storage-'+id),buttons=['refresh','clean','archive','drag-clean','drag-archive'].map(get);let alive=true,busy=false,directory=null,lastStorage=null,lastDrag=null,pendingAction=null;
 const controls=()=>{const locked=busy||!!pendingAction;buttons.forEach(button=>button.disabled=locked);get('archive').disabled=locked||!lastStorage?.residue_files;get('drag-clean').disabled=locked||!lastDrag?.files;get('drag-archive').disabled=locked||!lastDrag?.files;};
 const size=bytes=>(bytes/1048576).toFixed(1)+' MiB';
 const paint=status=>{lastStorage=status;get('usage').textContent='暂存占用 '+size(status.staged_bytes+status.pending_bytes)+' / '+size(status.limit_bytes);get('counts').textContent=status.protected_files+' 个待保存文件受保护，'+status.residue_files+' 个遗留文件。';controls();};
 async function run(action){
  if(!alive||busy||pendingAction)return;busy=true;controls();
  try{const result=await invoke('image_storage',{action});if(!alive)return;
   busy=false;paint(result.status||result);
   if(action==='clean')get('result').textContent='已清理 '+result.removed+' 个重复副本。';
   if(action==='archive'){get('result').textContent='已备份 '+result.archived+' 个文件，'+result.retained+' 个原文件仍保留。';if(result.directory){directory=result.directory;get('open').hidden=false;}}
  }catch(error){if(alive){busy=false;controls();fail(error);}}
 }
 get('refresh').onclick=async()=>{await run('status');await runDrag('drag-status');};get('clean').onclick=()=>run('clean');get('archive').onclick=()=>run('archive');
 async function runDrag(action){
  if(!alive||busy||pendingAction)return;busy=true;controls();
  try{const result=await invoke('image_storage',{action});if(!alive)return;const status=result.status||result;lastDrag=status;get('drag-usage').textContent='拖出副本 '+size(status.bytes)+' / '+size(status.limit_bytes)+'，共 '+status.files+' 个文件。';if(action==='drag-clean')get('result').textContent='已清理 '+result.removed+' 个拖出副本，'+result.retained+' 个文件仍保留。'+(result.deferred?'其中 '+result.deferred+' 个待下一批，可再次清理。':'');if(action==='drag-archive'){get('result').textContent='已备份 '+result.archived+' 个拖出文件，'+result.retained+' 个仍保留。占用中、超过 32 MiB 或未轮到本批的文件不会移除；恢复副本仍占磁盘空间。';if(result.directory){directory=result.directory;get('open').hidden=false;}}}
  catch(error){if(alive)fail(error)}finally{if(alive){busy=false;controls();}}
 }
 function confirmDrag(action,text){
  if(!alive||busy||pendingAction||!lastDrag?.files)return;
  pendingAction=action;get('confirm-text').textContent=text;get('confirm-accept').textContent=action==='drag-clean'?'确认清理':'确认备份并移出';get('confirm').hidden=false;controls();get('confirm').scrollIntoView({block:'nearest'});get('confirm-cancel').focus();
 }
 function dismissConfirm(){pendingAction=null;get('confirm').hidden=true;controls();}
 get('confirm').hidden=true;
 get('confirm-cancel').onclick=()=>{if(alive){dismissConfirm();get('drag-clean').focus();}};
 get('confirm-accept').onclick=()=>{if(!alive||busy||!pendingAction)return;const action=pendingAction;dismissConfirm();return runDrag(action);};
 get('drag-clean').onclick=()=>confirmDrag('drag-clean','请确认目标软件已完成复制。清理 QuickGen 的拖出副本？画板原图及目标文件夹中的图片不会删除。');
 get('drag-archive').onclick=()=>confirmDrag('drag-archive','请确认目标软件已完成复制。将未占用的拖出文件先备份校验，再移出拖出区？恢复副本仍占磁盘空间，原图不会删除。');
 get('drag-open').onclick=()=>{if(alive)invoke('board_drag_exports_open').catch(error=>{if(alive)fail(error)});};
 get('open').onclick=()=>{if(directory)invoke('board_recovery_open',{directory}).catch(error=>{if(alive)fail(error)});};
 get('back').onclick=()=>{alive=false;window.quickgenGatewaySettings.mount(content,invoke,fail).catch(fail);};
 await run('status');await runDrag('drag-status');
}};
