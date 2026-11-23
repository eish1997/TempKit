'use strict';
// Real board/IPC exercise; enabled only by diagnostic mode AND isolated profile.
globalThis.quickgenBoardReview={async run(api){
 const invoke=window.__TAURI__.core.invoke,result={ok:false,checks:[]};
 const assert=(value,message)=>{if(!value)throw Error(message)};
 const wait=async(test,label)=>{const until=Date.now()+25000;while(!test()){assert(Date.now()<until,'Timeout '+label);await new Promise(r=>setTimeout(r,100));await api.sync()}};
 const card=j=>document.querySelector('[data-job="'+j.id+'"]');
 const click=(j,text)=>{const button=[...card(j).querySelectorAll('button')].find(b=>b.textContent===text);assert(button,'Missing '+text);button.click()};
 const checkFailure=j=>{assert(j.failure?.stage==='archive','Missing archive failure');assert(card(j)?.textContent.includes('结果保存失败'),'Failure not visible');assert(card(j)?.textContent.includes('Isolated review: archive unavailable'),'Error not visible');assert([...card(j).querySelectorAll('button')].some(b=>b.textContent==='重试保存'),'Retry archive UI missing');const box=card(j).getBoundingClientRect();for(const button of card(j).querySelectorAll('button')){const b=button.getBoundingClientRect();assert(b.top>=box.top&&b.bottom<=box.bottom&&b.left>=box.left&&b.right<=box.right,'Button clipped outside card')}assert(parseFloat(getComputedStyle(card(j)).height)<=144,'Task card exceeds existing result spacing')};
 try{
  const phase=localStorage.getItem('review-phase')||'initial';
  await api.sync();await wait(()=>!api.busy(),'initial sync settled');
  if(phase==='initial'){
   await invoke('review_archive_fault',{enabled:true});api.simulate([api.items[0],api.items[0]],api.cards.get('clay'));
   await wait(()=>api.jobs.length===2&&api.jobs.every(j=>j.failure?.stage==='archive'&&card(j)?.textContent.includes('结果保存失败'))&&!api.busy(),'archive errors rendered');
   api.jobs.forEach(checkFailure);
   const before=(await invoke('task_snapshot'))[0];click(api.jobs[0],'重试保存');
   await wait(()=>!api.busy(),'sync');
   let after;for(let n=0;n<100;n++){after=(await invoke('task_snapshot'))[0];if(after.event_version>before.event_version)break;await new Promise(r=>setTimeout(r,100))}
   assert(after.event_version>before.event_version&&after.attempt===1,'Failed retry must publish newer error, same attempt');
   const counts=await invoke('review_archive_fault',{enabled:true});assert(counts.executions===2&&counts.placements===0,'Archive retry reran executor');
   click(api.jobs[0],'隐藏');await api.save();assert(!card(api.jobs[0]),'Hide failed');assert(api.items.length===1,'Original lost');
   localStorage.setItem('review-phase','reload-failed');location.reload();return;
  }
  if(phase==='reload-failed'){
   assert(api.jobs.length===2&&api.jobs[0].hidden&&!card(api.jobs[0]),'Hidden state lost on WebView reload');checkFailure(api.jobs[1]);
   assert((await invoke('review_archive_fault',{enabled:true})).executions===2,'Reload reran executor');
   localStorage.setItem('review-phase','restart-failed');result.checks.push('archive error and failed retry publish to real UI; hide and WebView reload retain state; executions=2');
   result.ok=true;result.flushExit=true;await invoke('board_smoke_finish',{result});return;
  }
  if(phase==='restart-failed'){
   assert(api.jobs[0].hidden,'Hidden state lost on process restart');checkFailure(api.jobs[1]);
   const counts=await invoke('review_archive_fault',{enabled:false});assert(counts.executions===2&&counts.placements===0,'Restart auto-executed or archived failed task');
   document.getElementById('receipt-show').click();api.jobs.forEach(checkFailure);
   const first=api.jobs[0],second=api.jobs[1];click(first,'重试保存');
   await wait(()=>first.state==='done'&&api.items.length===2,'UI retry archive result placement');assert(!first.failure,'Success retained failure');
   const firstResult=api.items.find(i=>i.taskId===first.hostTaskId);assert(firstResult,'Missing first result');
   document.querySelector('[data-image="'+firstResult.id+'"] .remove').click();await api.save();
   // Reproduce the old released client's lost local row, then archive while no
   // row exists. Suppress no host state: next sync must recover from host feed.
   await wait(()=>!api.busy(),'settle');api.jobs.splice(api.jobs.indexOf(second),1);await api.save();
   const task=(await invoke('task_snapshot')).find(t=>t.id===second.hostTaskId);
   await invoke('task_control',{command:{task_id:task.id,attempt:task.attempt,event_version:task.event_version,action:'retry_archive'}});
   await api.sync();await wait(()=>api.items.some(i=>i.taskId===second.hostTaskId),'host-only output');
   assert(!api.items.some(i=>i.id===firstResult.id),'Deleted result resurrected');assert(api.items.some(i=>i.id==='local-1'),'Original lost');
   localStorage.setItem('review-deleted',firstResult.id);localStorage.setItem('review-phase','reload-success');await api.save();location.reload();return;
  }
  if(phase==='reload-success'||phase==='restart-success'){
   await api.sync();const counts=await invoke('review_archive_fault',{enabled:false});
   assert(counts.executions===2&&counts.placements===2,'Execution/placement duplicate');
   assert(api.items.length===2&&api.items.some(i=>i.id==='local-1'),'Original or output missing');assert(!api.items.some(i=>i.id===localStorage.getItem('review-deleted')),'Explicit deletion resurrected');
   assert((await invoke('task_snapshot')).every(t=>t.state==='succeeded'&&t.attempt===1&&!t.failure),'Terminal failure not cleared');
   await Promise.all([...document.querySelectorAll('#world img')].map(img=>img.decode()));
   result.checks.push('UI retry saves without re-execution; host-only output restored; deleted result stays deleted; original retained; executions=2, receipts=2');
   result.ok=true;
   if(phase==='reload-success'){localStorage.setItem('review-phase','restart-success');result.flushExit=true}else{result.restart=true;result.checks.push('WebView reload and process restart preserve outputs, deletion receipts and cleared errors')}
   await invoke('board_smoke_finish',{result});return;
  }
  throw Error('Unknown review phase');
 }catch(error){result.error=String(error);result.cards=[...document.querySelectorAll('.generation-job')].map(n=>n.textContent);result.jobs=api.jobs.map(j=>({id:j.id,state:j.state,failure:j.failure,hidden:j.hidden}));result.phase=localStorage.getItem('review-phase');result.notice=document.getElementById('notice')?.textContent;await invoke('board_smoke_finish',{result})}
}};
