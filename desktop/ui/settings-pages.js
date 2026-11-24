'use strict';
window.quickgenSettingsPages={
 home(){return {title:'设置',mount:async ctx=>{
  for(const [id,title,page] of [['connections','模型连接',this.connections()],['budget','调用额度',this.budget()],['storage','存储与恢复',this.storage()]]){
   const button=document.createElement('button');button.id='settings-'+id;button.className='settings-entry';button.textContent=title;button.onclick=()=>ctx.push(page);ctx.root.append(button);
  }
 }};},
 legacy(title,module,backId){return {title,mount:async ctx=>{
  await module.mount(ctx.root,ctx.invoke,ctx.fail,{navigation:true,onDispose:ctx.onDispose});
  if(!ctx.active())return;
  const back=ctx.root.querySelector('#'+backId);if(back)back.hidden=true;
 }};},
 connections(){return window.quickgenConnectionPages.list();},
 budget(){return this.legacy('调用额度',window.quickgenImageBudget,'budget-back');},
 storage(){return {title:'存储与恢复',mount:async ctx=>{
  for(const [section,title] of [['results','结果暂存'],['drag','拖出副本'],['recovery','完整恢复副本']]){
   const button=document.createElement('button');button.id='storage-'+section;button.className='settings-entry';button.textContent=title;
   button.onclick=()=>ctx.push(section==='recovery'?this.recovery():{title,mount:child=>window.quickgenImageStorage.mount(child.root,child.invoke,child.fail,{section,onDispose:child.onDispose})});ctx.root.append(button);
  }
 }};},
 recovery(){return {title:'完整恢复副本',mount:async ctx=>{
  const text=document.createElement('p');text.textContent='等待画板、任务与便签保存后，生成并验证完整恢复副本。副本仍占磁盘空间。';ctx.root.append(text);
  const button=document.createElement('button'),status=document.createElement('p'),open=document.createElement('button');button.textContent='保存完整恢复副本';open.textContent='打开恢复目录';open.hidden=true;status.setAttribute('role','status');ctx.root.append(button,status,open);
  let directory=null;button.disabled=true;
  const refresh=document.createElement('button');refresh.textContent='刷新状态';ctx.root.append(refresh);
  const read=async()=>{refresh.disabled=true;try{const state=await ctx.invoke('manager_recovery_status');button.disabled=state.pending;status.textContent=state.pending?'恢复副本正在生成；稍后刷新查看。':state.last?.error?'上次恢复副本失败：'+state.last.error:state.last?.result?.directory?'上次恢复副本已验证并保存。':'尚未在本次运行中生成恢复副本。';directory=state.last?.result?.directory||null;open.hidden=!directory;}catch(error){ctx.fail(error);}finally{refresh.disabled=false;}};
  refresh.onclick=read;
  button.onclick=async()=>{if(button.disabled)return;button.disabled=true;status.textContent='正在等待保存并生成恢复副本…';try{const result=await ctx.invoke('manager_recovery_request',{requestId:crypto.randomUUID()});if(!result?.directory)throw Error('未收到已验证的恢复目录');directory=result.directory;status.textContent='恢复副本已验证并保存。';open.hidden=false;}catch(error){ctx.fail(error);status.textContent='恢复副本未确认完成，请返回后核对。';}};
  open.onclick=()=>{if(directory)ctx.invoke('board_recovery_open',{directory}).catch(ctx.fail);};
  await read();
 }};}

};
