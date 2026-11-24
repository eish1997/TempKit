'use strict';
window.quickgenConnectionPages={
 models:new Map(),
 label(c){return (c.account_id||'未知账号')+' · '+(c.base_url||'地址不可用');},
 status(c){return ({ready:'已通过认证',unverified:'尚未验证',revoked:'已撤销',offline:'连接离线'})[c.status]||'状态未知';},
 button(ctx,id,text,action){const b=document.createElement('button');b.type='button';b.id=id;b.textContent=text;b.onclick=action;ctx.root.append(b);return b;},
 list(){return {title:'模型连接',mount:async ctx=>{
  this.button(ctx,'connection-add','添加连接',()=>ctx.push(this.add()));
  const status=document.createElement('p');status.textContent='正在读取连接…';ctx.root.append(status);
  try{const rows=await ctx.invoke('gateway_connections');status.textContent=rows.length?'':'尚无连接，添加后可用于预设模型设置。';
   for(const c of rows){const duplicates=rows.filter(row=>this.label(row)===this.label(c)).length>1;
    const b=this.button(ctx,'connection-'+c.id,this.label(c)+(duplicates?' · '+c.id.slice(-8):'')+' — '+this.status(c),()=>ctx.push(this.detail(c.id)));b.className='settings-entry';}
  }catch(e){ctx.fail(e);status.textContent='连接读取失败，请返回后重试。';}
 }};},
 add(){const draft={address:'http://127.0.0.1:3000',account:''};return {title:'添加连接',mount:async ctx=>{
  ctx.root.innerHTML='<form autocomplete="off"><label>New API 地址<input name="address" type="url" required maxlength="512"></label><label>账号名称（英文或数字）<input name="account" required maxlength="64" pattern="[A-Za-z0-9_.-]+"></label><label>访问令牌<input name="token" type="password" required maxlength="4096" autocomplete="new-password"></label><p>保存创建一个新连接，不调用模型。返回或关闭会放弃未保存内容；令牌不会保留。</p><button type="submit">保存连接</button><p role="status"></p></form>';
  const form=ctx.root.querySelector('form'),button=form.querySelector('button'),status=form.querySelector('p[role]');
  form.elements.address.value=draft.address;form.elements.account.value=draft.account;
  let attempted=false;
  form.onsubmit=async event=>{event.preventDefault();if(button.disabled||attempted)return;button.disabled=true;attempted=true;
   const id=crypto.randomUUID(),profile={schema_version:1,profile_id:'profile-'+id,connection_id:'gateway-'+id,account_id:form.elements.account.value.trim(),base_url:form.elements.address.value.trim()};
   let token=form.elements.token.value;form.elements.token.value='';
   try{await ctx.invoke('gateway_configure',{profile,token});status.textContent='连接已保存，请返回列表进行检测。';}
   catch(e){ctx.fail(e);status.textContent='未确认保存结果，请返回列表核对后再操作，避免重复创建。';}
   finally{token='';}
  };
 }};},
 detail(id){return {title:'连接详情',mount:async ctx=>{
  const rows=await ctx.invoke('gateway_connections'),connection=rows.find(c=>c.id===id);if(!connection)throw Error('连接不存在，请返回列表。');
  const info=document.createElement('p');info.textContent=this.label(connection);ctx.root.append(info);
  const identity=document.createElement('details'),summary=document.createElement('summary'),internal=document.createElement('p');summary.textContent='内部标识';internal.textContent=id;identity.append(summary,internal);ctx.root.append(identity);
  const status=document.createElement('p');status.setAttribute('role','status');status.textContent=this.status(connection);ctx.root.append(status);
  const models=document.createElement('details'),caption=document.createElement('summary'),list=document.createElement('div');caption.textContent='可用模型（尚未检测）';models.append(caption,list);ctx.root.append(models);
  const paint=items=>{caption.textContent='可用模型 · '+items.length;list.replaceChildren();for(const model of items){const field=document.createElement('textarea');field.readOnly=true;field.rows=2;field.className='model-id';field.value=model;field.setAttribute('aria-label','模型 ID，可选择复制');list.append(field);}};
  const cached=this.models.get(id);if(cached&&cached.reference===connection.credential_ref)paint(cached.items);
  if(connection.status!=='revoked'){
   const probe=this.button(ctx,'connection-probe','检测连接',async()=>{probe.disabled=true;status.textContent='正在检测认证与模型列表…';try{const receipt=await ctx.invoke('gateway_probe',{id});if(!receipt?.authenticated||!Array.isArray(receipt.models))throw Error('无效的检测结果');this.models.set(id,{reference:connection.credential_ref,items:receipt.models});status.textContent='认证通过 · 可见 '+receipt.models.length+' 个模型（不代表图片生成已验证）';paint(receipt.models);}catch(e){ctx.fail(e);status.textContent='检测失败，请检查连接后重试。';}finally{probe.disabled=false;}});
   this.button(ctx,'connection-rotate','更换令牌',()=>ctx.push(this.rotation(id)));
  }
  const warning=document.createElement('p');warning.textContent='撤销后，引用此连接的预设不能继续发出请求；已有任务与结果保留，已发出的请求不会取消。';ctx.root.append(warning);
  const remove=this.button(ctx,'connection-revoke','撤销连接',()=>{remove.hidden=true;confirm.hidden=false;});remove.hidden=connection.status==='revoked';
  const confirm=document.createElement('div');confirm.hidden=true;const yes=document.createElement('button'),no=document.createElement('button');yes.textContent='确认撤销';no.textContent='取消';confirm.append(no,yes);ctx.root.append(confirm);no.onclick=()=>{confirm.hidden=true;remove.hidden=false;};
  const revoke=async()=>{yes.disabled=true;no.disabled=true;retry.disabled=true;try{await ctx.invoke('gateway_revoke',{id});status.textContent='连接已撤销，凭据已清除。';confirm.hidden=true;retry.hidden=true;this.models.delete(id);for(const key of ['connection-probe','connection-rotate']){const control=ctx.root.querySelector('#'+key);if(control)control.disabled=true;}}catch(e){ctx.fail(e);confirm.hidden=true;const latest=await ctx.invoke('gateway_connections').catch(()=>null);const revoked=latest?.find(c=>c.id===id)?.status==='revoked';status.textContent=revoked?'连接已撤销；凭据清除未完成。':'结果未确认，请返回列表核对。';retry.hidden=!revoked;}finally{retry.disabled=false;}};
  const retry=this.button(ctx,'connection-clear-retry','重试清除凭据',()=>revoke());retry.hidden=!connection.credential_cleanup_pending;yes.onclick=revoke;
 }};},
 rotation(id){return {title:'更换令牌',mount:async ctx=>{
  const rows=await ctx.invoke('gateway_connections'),connection=rows.find(c=>c.id===id);if(!connection||connection.status==='revoked')throw Error('连接不可用');
  await window.quickgenGatewayRotation.mount(ctx.root,ctx.invoke,ctx.fail,connection,{onDispose:ctx.onDispose,requireReopenOnFailure:true});if(!ctx.active())return;
  const back=ctx.root.querySelector('#rotation-back');back.hidden=true;
  ctx.onDispose(()=>this.models.delete(id));
 }};}
};
