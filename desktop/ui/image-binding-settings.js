'use strict';
window.quickgenImageBindingSettings={async mount(content,invoke,fail){
 content.innerHTML='<form><label>面板预设<select name="preset" required></select></label><label>已验证连接<select name="connection" required></select></label><label>准确模型 ID<input name="model" required maxlength="256" spellcheck="false"></label><label>尺寸（可留空）<input name="size" maxlength="256" placeholder="按渠道说明填写"></label><label>质量（可留空）<input name="quality" maxlength="256" placeholder="按渠道说明填写"></label><label>返回方式<select name="format"><option value="">使用服务默认值</option><option value="b64_json">Base64 图片</option><option value="url">图片地址</option></select></label><p>保存只绑定模型，不调用生图。尺寸、质量须符合渠道说明；模型列表可见不代表已支持图片编辑。需完成费用核验并设置调用额度后才能执行。</p><div class="actions"><button type="button" id="binding-back">返回连接</button><button type="button" id="binding-remove" disabled>解除模型绑定</button><button type="submit">保存绑定</button></div><p id="binding-status" role="status"></p></form>';
 const form=content.querySelector('form'),status=content.querySelector('#binding-status'),button=form.querySelector('[type=submit]'),remove=content.querySelector('#binding-remove');
 let alive=true,sequence=0,busy=false,loaded=false,current=null;
 content.querySelector('#binding-back').onclick=()=>{alive=false;window.quickgenGatewaySettings.mount(content,invoke,fail).catch(fail);};
 const [config,connections]=await Promise.all([invoke('load_config'),invoke('gateway_connections')]);if(!alive)return;
 const cards=config.abilities||[];
 const add=(select,value,label)=>{const option=document.createElement('option');option.value=value;option.textContent=label;select.append(option);return option;};
 for(const card of cards)add(form.elements.preset,card.id,card.name+(card.imageBinding?' · 已绑定':''));
 // Keep historical connections visible so a revoked binding remains inspectable.
 for(const connection of connections)add(form.elements.connection,connection.id,connection.id+(connection.status==='ready'?'':' · 未验证或已撤销'));
 form.elements.connection.value=connections.find(row=>row.status==='ready')?.id||'';
 function controls(){button.disabled=busy||!loaded||!current||!connections.some(row=>row.id===form.elements.connection.value&&row.status==='ready');remove.disabled=busy||!current?.imageBinding;form.elements.preset.disabled=busy;}
 async function show(){
  const ticket=++sequence;current=cards.find(card=>card.id===form.elements.preset.value)||null;loaded=false;
  form.elements.model.value='';form.elements.size.value='';form.elements.quality.value='';form.elements.format.value='';status.textContent='';controls();
  if(!current){status.textContent='请先添加面板预设。';return;}
  const card=current;
  if(!card.imageBinding){loaded=true;status.textContent='未绑定模型；当前使用模拟执行。';controls();return;}
  try{
   const binding=await invoke('gateway_image_binding',{reference:card.imageBinding});if(!alive||ticket!==sequence)return;
   form.elements.connection.value=binding.capability.reference.connection_id;
   form.elements.model.value=binding.policy.model;form.elements.size.value=card.imageParameters?.size||'';form.elements.quality.value=card.imageParameters?.quality||'';form.elements.format.value=binding.policy.response_format||'';
   loaded=true;status.textContent=connections.some(row=>row.id===form.elements.connection.value&&row.status==='ready')?'已加载保存的模型绑定。':'原连接未验证或已撤销，可选择其他连接或解除绑定。';
  }catch(error){if(alive&&ticket===sequence){status.textContent='原绑定读取失败，可解除绑定后重新配置。';fail(error);}}
  if(alive&&ticket===sequence)controls();
 }
 form.elements.preset.onchange=()=>show().catch(fail);form.elements.connection.onchange=controls;
 remove.onclick=async()=>{
  if(remove.disabled)return;const card=current;busy=true;controls();
  try{await invoke('gateway_unbind_preset',{presetId:card.id,expected:card.imageBinding});delete card.imageBinding;delete card.imageParameters;await show();status.textContent='已解除模型绑定，预设恢复模拟执行；已有任务与结果保留。';}
  catch(error){fail(error);}finally{busy=false;controls();}
 };
 form.onsubmit=async event=>{
  event.preventDefault();if(button.disabled)return;busy=true;controls();status.textContent='';const card=current;
  const model=form.elements.model.value.trim(),size=form.elements.size.value.trim(),quality=form.elements.quality.value.trim();
  const parameters={};if(size)parameters.size=size;if(quality)parameters.quality=quality;
  try{const binding=await invoke('gateway_bind_preset',{connectionId:form.elements.connection.value,presetId:card.id,policy:{model,sizes:size?[size]:[],qualities:quality?[quality]:[],response_format:form.elements.format.value||null},parameters});card.imageBinding=binding.capability.reference;card.imageParameters=parameters;status.textContent='模型绑定已保存；未调用模型。';}
  catch(error){fail(error);}finally{busy=false;controls();}
 };
 await show();
}};
