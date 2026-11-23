'use strict';
globalThis.quickgenGenerationRecovery={async run(api){
 const invoke=window.__TAURI__.core.invoke,result={ok:false,checks:[]};
 const assert=(v,m)=>{if(!v)throw Error(m)};
 const wait=async(test,label)=>{const until=Date.now()+25000;while(!test()){assert(Date.now()<until,'Timeout '+label);await new Promise(r=>setTimeout(r,100));await api.sync()}};
 const stage=localStorage.getItem('generation-recovery-stage')||'initial';
 try{
  const performanceMode=(await invoke('generation_test',{action:'performance'})).mode;
  if(performanceMode){await new Promise((resolve,reject)=>{const script=document.createElement('script');script.src='generation-performance.js';script.onload=resolve;script.onerror=reject;document.head.append(script)});await quickgenGenerationPerformance.run(api,performanceMode);return;}
  await invoke('board_capacity_progress',{result:{step:'run',stage}});
  await api.sync();await wait(()=>!api.busy(),'initial sync');
  if(stage==='initial'){
   const emptyBudget=await invoke('image_budget',{});assert(emptyBudget.limit_micros===null&&emptyBudget.reserved_micros===0,'Unexpected initial budget');
   const savedBudget=await invoke('image_budget',{limitMicros:2});assert(savedBudget.limit_micros===2,'Budget not saved');
   let resetRejected=false;try{await invoke('image_budget',{limitMicros:3})}catch(_){resetRejected=true}assert(resetRejected,'Budget reset was accepted');
   assert((await invoke('task_snapshot')).length===0,'Budget settings submitted a task');
   const profile={schema_version:1,profile_id:'rotation-profile',connection_id:'rotation-connection',account_id:'synthetic-account',base_url:'http://127.0.0.1:1'};
   const original=await invoke('gateway_configure',{profile,token:'synthetic-rotation-old'});
   await new Promise((resolve,reject)=>{const script=document.createElement('script');script.src='gateway-rotation.js';script.onload=resolve;script.onerror=reject;document.head.append(script)});
   const rotationContent=document.createElement('div'),rotationErrors=[];
   await window.quickgenGatewayRotation.mount(rotationContent,invoke,error=>rotationErrors.push(String(error)),original);
   const rotationForm=rotationContent.querySelector('form');rotationForm.elements.account.value='different-account';rotationForm.elements.token.value='synthetic-wrong-account';
   await rotationForm.onsubmit({preventDefault(){}});assert(rotationErrors.length===1&&rotationForm.elements.token.value==='','Rotation mismatch or secret clearing failed');
   rotationForm.elements.account.value=profile.account_id;rotationForm.elements.token.value='synthetic-rotation-new';
   await rotationForm.onsubmit({preventDefault(){}});assert(rotationErrors.length===1&&rotationForm.querySelector('[type=submit]').disabled,'Rotation form failed to save');
   const rotated=(await invoke('gateway_connections')).find(c=>c.id===original.id);
   assert(rotated.status==='unverified'&&rotated.profile_id===original.profile_id&&rotated.credential_ref!==original.credential_ref,'Rotation changed identity or kept readiness');
   let staleRejected=false;try{await invoke('gateway_rotate',{id:original.id,expectedRef:original.credential_ref,accountId:profile.account_id,token:'synthetic-stale'})}catch(_){staleRejected=true}assert(staleRejected,'Stale rotation accepted');
   assert((await invoke('task_snapshot')).length===0,'Rotation submitted a task');localStorage.setItem('generation-rotation-ref',rotated.credential_ref);
   await invoke('generation_test',{action:'setup'});
   const fixtureConnection=(await invoke('gateway_connections')).find(c=>c.id==='fixture');
   await invoke('generation_test',{action:'pause-scheduling'});
   api.simulate([api.items[0]],api.cards.get('clay'));
   await wait(()=>api.jobs[0]?.hostTaskId,'authenticated task admission before rotation');
   await invoke('gateway_rotate',{id:'fixture',expectedRef:fixtureConnection.credential_ref,accountId:'synthetic',token:'synthetic-fixture-rotated'});
   await invoke('generation_test',{action:'resume-scheduling'});
   await new Promise(resolve=>setTimeout(resolve,500));await api.sync();
   const queuedTasks=await invoke('task_snapshot'),waitingStats=await invoke('generation_test',{action:'fault'});
   assert(queuedTasks.length===1&&queuedTasks[0].state==='queued'&&queuedTasks[0].attempt===1&&!queuedTasks[0].failure,'Scheduler failed a task waiting for authentication');
   assert(waitingStats.executions===0&&waitingStats.requests===0&&waitingStats.budget.reserved_micros===0,'Unverified rotation dispatched or reserved');
   const authenticated=await invoke('gateway_probe',{id:'fixture'});assert(authenticated.authenticated&&authenticated.models.includes('fixture-image'),'Rotated fixture did not authenticate');
   // Only the successful versioned authentication callback wakes the resource
   // worker here: no new submission, task control or retry-save operation.

   await wait(()=>api.work()?.save_failed,'received result save failure');
   const drag=await invoke('generation_test',{action:'drag-fixture'});assert(drag.files===2,'Missing drag fixtures');
   const dragClean=await invoke('image_storage',{action:'drag-clean'});assert(dragClean.removed===1&&dragClean.retained===1,'Drag cleanup damaged partial or failed to remove copy');
   const dragBackup=await invoke('image_storage',{action:'drag-archive'});assert(dragBackup.archived===1&&dragBackup.retained===0&&dragBackup.status.files===0,'Drag backup did not preserve and remove residue');
   const storage=await invoke('image_storage',{action:'status'});assert(storage.protected_files===1&&storage.residue_files===1,'Incorrect staging classification');
   const archived=await invoke('image_storage',{action:'archive'});assert(archived.archived===1&&archived.retained===0&&archived.status.protected_files===1,'Maintenance removed protected output or failed to archive residue');
   const cleaned=await invoke('image_storage',{action:'clean'});assert(cleaned.removed===0&&cleaned.status.protected_files===1,'Cleanup removed unsaved result');
   assert(document.getElementById('hint-action').textContent==='重试保存','Missing actual retry control');
   assert(document.getElementById('hint-text').textContent.includes('图片结果尚未保存'),'Missing save failure text');
   const job=api.jobs[0];await wait(()=>!api.busy(),'failure DOM committed');const card=document.querySelector('[data-job="'+job.id+'"]');
   assert(card.textContent.includes('结果保存失败')&&!card.textContent.includes('模拟生成'),'Real task mislabeled');
   assert(!document.getElementById('receipt').textContent.includes('逐张模拟'),'Real receipt mislabeled');
   const hide=[...card.querySelectorAll('button')].find(b=>b.textContent==='隐藏');assert(hide,'Missing hide');hide.click();await api.save();
   localStorage.setItem('generation-recovery-stage','reload-failed');location.reload();return;
  }
  if(stage!=='initial'){
   const rotated=(await invoke('gateway_connections')).find(c=>c.id==='rotation-connection');
   assert(rotated?.credential_ref===localStorage.getItem('generation-rotation-ref')&&rotated.status==='unverified'&&rotated.profile_id==='rotation-profile','Rotation lost across reload/restart');
  }
  if(stage==='reload-failed'){
   await wait(()=>api.work()?.save_failed,'reload retains save failure');assert(api.jobs[0].hidden,'Hidden state lost');
   const stats=await invoke('generation_test',{action:'fault'});assert(stats.executions===1&&stats.requests===1&&stats.downloads===1,'Reload resubmitted or redownloaded');
   localStorage.setItem('generation-recovery-stage','restart-failed');await api.save();
   result.ok=true;result.checks.push('real WebView received save error, retry control, hide and reload; one localhost request');
   // Normal exit must accept the verified receipt without regenerating; prior
   // crash/restart coverage remains in host tests and archived fixture evidence.
   result.flushExit=true;
   await invoke('board_smoke_finish',{result});return;
  }
  if(stage==='restart-failed'){
   await wait(()=>api.work()?.save_failed,'restart preserves failure and retry');assert(api.jobs[0].hidden,'Restart lost hidden state');
   const note=await invoke('note_create');assert(note.id,'Notes unavailable during result fault');
   await invoke('generation_test',{action:'download-start'});
   const firstRetry=document.getElementById('hint-action');assert(!firstRetry.hidden&&firstRetry.textContent==='重试保存','Download retry unavailable');firstRetry.click();
   let downloadStats;const downloadDeadline=Date.now()+25000;
   do{await new Promise(r=>setTimeout(r,100));await api.sync();downloadStats=await invoke('generation_test',{action:'fault'});assert(Date.now()<downloadDeadline,'Download retry failure timeout');}while(downloadStats.downloads!==2||!api.work()?.save_failed||api.busy());
   assert(downloadStats.requests===1&&downloadStats.executions===1,'Download failure regenerated');
   await invoke('generation_test',{action:'download-clear'});
   await invoke('generation_test',{action:'archive-fault'});
   const before=await invoke('generation_test',{action:'clear'});assert(before.executions===1&&before.requests===1,'Restart resubmitted');
   const button=document.getElementById('hint-action');assert(!button.hidden&&button.textContent==='重试保存','Retry control unavailable');button.click();
   await wait(()=>api.jobs[0].failure?.stage==='archive','background archive publishes failure');
   document.getElementById('receipt-show').click();await wait(()=>!api.busy(),'archive error rendered');
   const retryArchive=()=>{const card=document.querySelector('[data-job="'+api.jobs[0].id+'"]');assert(card?.textContent.includes('结果保存失败'),'Missing archive error UI');const retry=[...card.querySelectorAll('button')].find(b=>b.textContent==='重试保存');assert(retry,'Missing archive retry');retry.click();};
   const failedTask=(await invoke('task_snapshot'))[0];retryArchive();
   const deadline=Date.now()+25000;let failedAgain;
   do{await new Promise(r=>setTimeout(r,100));await api.sync();failedAgain=(await invoke('task_snapshot'))[0];assert(Date.now()<deadline,'Archive retry timeout');}while(!failedAgain.failure||failedAgain.event_version<=failedTask.event_version);
   assert(failedAgain.attempt===1,'Archive retry changed attempt');
   await wait(()=>!api.busy()&&api.jobs[0].failure?.stage==='archive'&&document.querySelector('[data-job="'+api.jobs[0].id+'"]')?.textContent.includes('结果保存失败'),'retried archive error rendered');
   await invoke('generation_test',{action:'archive-clear'});retryArchive();
   await wait(()=>api.items.length===2&&api.jobs[0].state==='done','actual button saves, scheduler archives and board places result');
   assert(!api.work(),'Failure work not cleared');
   const output=api.items.find(i=>i.taskId===api.jobs[0].hostTaskId);assert(output,'No host result placement');
   await wait(()=>!api.busy()&&document.querySelector('[data-image="'+output.id+'"] img'),'result DOM committed');
   await document.querySelector('[data-image="'+output.id+'"] img').decode();
   document.querySelector('[data-image="'+output.id+'"] .remove').click();await api.save();
   await invoke('board_capacity_progress',{result:{step:'deleted-and-saved',stage}});
   localStorage.setItem('generation-recovery-deleted',output.id);localStorage.setItem('generation-recovery-stage','reload-success');location.reload();return;
  }
  if(stage==='reload-success'||stage==='restart-success'){
   const stats=await invoke('generation_test',{action:'clear'}),tasks=await invoke('task_snapshot');
   assert(stats.executions===1&&stats.requests===1&&stats.downloads===3&&stats.budget.reserved_micros===1,'Repeated execution, download or reservation');
   assert(tasks.length===1&&tasks[0].state==='succeeded'&&tasks[0].attempt===1&&!tasks[0].failure,'Wrong terminal state');
   assert(api.items.length===1&&api.items[0].id==='local-1','Original lost or deletion resurrected');
   assert(!api.items.some(i=>i.id===localStorage.getItem('generation-recovery-deleted')),'Deleted output resurrected');
   await invoke('board_capacity_progress',{result:{step:'verified-terminal',stage}});
   result.ok=true;result.stats=stats;result.checks.push('save-only retry via real button, automatic archive and placement, no duplicate request, deletion persists');
   if(stage==='reload-success'){localStorage.setItem('generation-recovery-stage','restart-success');result.flushExit=true}else{result.restart=true}
   await invoke('board_smoke_finish',{result});return;
  }
  throw Error('Unknown fixture phase');
 }catch(error){result.error=String(error);result.stage=stage;result.notice=document.getElementById('notice')?.textContent;result.work=api.work();result.jobs=api.jobs.map(j=>({state:j.state,hidden:j.hidden,failure:j.failure}));await invoke('board_smoke_finish',{result})}
}};
