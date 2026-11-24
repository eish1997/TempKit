'use strict';
window.quickgenGatewayRotation={async mount(content,invoke,fail,connection,options={}){
 content.innerHTML='<p id="rotation-target" style="overflow-wrap:anywhere"></p><form autocomplete="off"><label>原账号名称<input name="account" required maxlength="64" pattern="[A-Za-z0-9_.-]+"></label><label>同账号的新令牌<input name="token" type="password" required maxlength="4096" autocomplete="new-password" spellcheck="false"></label><p>仅更换同一个 New API 账号的令牌，地址和任务目标不变。已开始的请求继续使用原令牌；待执行任务须在新令牌检测通过后继续。不同账号请返回新建连接。账号一致性由你确认，连接检测不证明令牌所属账号。</p><p id="rotation-status" role="status"></p><div class="actions"><button type="button" id="rotation-back">返回连接</button><button type="submit">保存新令牌</button></div></form>';
 content.querySelector('#rotation-target').textContent='连接：'+connection.id;
 const form=content.querySelector('form'),button=form.querySelector('[type=submit]'),status=content.querySelector('#rotation-status');let alive=true,busy=false,saved=false;
 const controls=()=>{button.disabled=busy||saved;form.elements.account.disabled=busy||saved;form.elements.token.disabled=busy||saved;};
 if(options.onDispose)content.querySelector('#rotation-back').hidden=true;
 content.querySelector('#rotation-back').onclick=()=>{alive=false;form.elements.token.value='';window.quickgenGatewaySettings.mount(content,invoke,fail).catch(fail);};
 form.onsubmit=async event=>{
  event.preventDefault();if(!alive||busy||saved)return;
  const accountId=form.elements.account.value.trim();if(!accountId){fail('请填写原连接的账号名称。');return;}
  let token=form.elements.token.value;form.elements.token.value='';busy=true;controls();
  try{await invoke('gateway_rotate',{id:connection.id,expectedRef:connection.credential_ref,accountId,token});if(alive){saved=true;status.textContent='新令牌已保存。请返回连接并重新检测；保存没有调用模型。';}}
  catch(error){if(options.requireReopenOnFailure)saved=true;if(alive){status.textContent='未确认保存成功，请返回连接核对当前状态后重试。';fail(error);}}
  finally{token='';if(alive){busy=false;controls();}}
 };
 if(options.onDispose)options.onDispose(()=>{alive=false;form.elements.token.value='';});
 controls();
}};
