'use strict';
window.quickgenGatewaySettings={async mount(content,invoke,fail){
 content.innerHTML='<form autocomplete="off"><label>New API 地址<input name="address" type="url" required value="http://127.0.0.1:3000" maxlength="512"></label><label>账号名称（英文或数字）<input name="account" required maxlength="64" pattern="[A-Za-z0-9_.-]+" placeholder="例如：local-test"></label><label>访问令牌<input name="token" type="password" required maxlength="4096" autocomplete="new-password" spellcheck="false"></label><p>保存不会调用模型或产生费用。更换地址或账号将新建连接。</p><div class="actions"><button type="submit">保存连接</button></div></form><div id="gateway-list" aria-live="polite"></div>';
 const form=content.querySelector('form'),list=content.querySelector('#gateway-list');
 async function refresh(){
  const connections=await invoke('gateway_connections');list.replaceChildren();
  for(const connection of connections){
   const row=document.createElement('div');row.className='row';const info=document.createElement('div'),name=document.createElement('strong'),status=document.createElement('p');
   name.textContent=connection.id;status.textContent=({unverified:'已保存 · 尚未验证',ready:'连接可用',offline:'连接离线',revoked:'已撤销'})[connection.status]||'状态未知';info.append(name,status);row.append(info);
   if(connection.status!=='revoked'){
    const probe=document.createElement('button');probe.type='button';probe.textContent='检测连接';probe.onclick=async()=>{
     probe.disabled=true;status.textContent='正在读取可用模型，不调用生图…';
     try{const receipt=await invoke('gateway_probe',{id:connection.id});
      if(!receipt?.authenticated||!Array.isArray(receipt.models))throw Error('未收到有效检测结果');
      status.textContent=receipt.models.length?'认证通过；图片能力尚未验证。模型：'+receipt.models.join('、'):'认证通过，但没有可用模型。';
     }catch(error){status.textContent='连接检测失败';fail(error);}finally{probe.disabled=false;}
    };info.append(probe);
    const rotate=document.createElement('button');rotate.type='button';rotate.textContent='更换同账号令牌';rotate.onclick=()=>window.quickgenGatewayRotation.mount(content,invoke,fail,connection).catch(fail);info.append(rotate);
   }
   const remove=document.createElement('button');remove.type='button';remove.textContent=connection.status==='revoked'?'重试清除凭据':'撤销连接';remove.onclick=async()=>{remove.disabled=true;try{await invoke('gateway_revoke',{id:connection.id});await refresh();}catch(error){fail(error);remove.disabled=false;}};row.append(remove);list.append(row);
  }
 const bind=document.createElement('button');bind.type='button';bind.textContent='预设模型绑定';bind.onclick=()=>window.quickgenImageBindingSettings.mount(content,invoke,fail).catch(fail);list.append(bind);
 const storage=document.createElement('button');storage.type='button';storage.textContent='暂存空间';storage.onclick=()=>window.quickgenImageStorage.mount(content,invoke,fail).catch(fail);list.append(storage);
 const budget=document.createElement('button');budget.type='button';budget.textContent='调用额度';budget.onclick=()=>window.quickgenImageBudget.mount(content,invoke,fail).catch(fail);list.append(budget);
 }
 form.onsubmit=async event=>{
  event.preventDefault();const button=form.querySelector('[type=submit]');if(button.disabled)return;
  const base_url=form.elements.address.value.trim(),account=form.elements.account.value.trim();if(!account)return;
  const id=crypto.randomUUID();const profile={schema_version:1,profile_id:'profile-'+id,connection_id:'gateway-'+id,account_id:account,base_url};
  // Plaintext is submitted once, never persisted by the WebView or returned by IPC.
  let token=form.elements.token.value;form.elements.token.value='';button.disabled=true;
  try{await invoke('gateway_configure',{profile,token});await refresh();}catch(error){fail(error);}finally{token='';button.disabled=false;}
 };
 await refresh();
}};
