'use strict';
window.quickgenImageBindingSettings={async mount(content,invoke,fail,options={}){
 content.innerHTML='<form><label>面板预设<select name="preset" required></select></label><label>已验证连接<select name="connection" required></select></label><label>准确模型 ID<input name="model" required maxlength="256" spellcheck="false"></label><label>尺寸（可留空）<input name="size" maxlength="256" placeholder="按渠道说明填写"></label><label>质量（可留空）<input name="quality" maxlength="256" placeholder="按渠道说明填写"></label><label>返回方式<select name="format"><option value="">使用服务默认值</option><option value="b64_json">Base64 图片</option><option value="url">图片地址</option></select></label><p>保存只绑定模型，不调用生图。尺寸、质量须符合渠道说明；模型列表可见不代表已支持图片编辑。需完成费用核验并设置调用额度后才能执行。</p><div class="actions"><button type="button" id="binding-back">返回连接</button><button type="button" id="binding-remove" disabled>解除模型绑定</button><button type="submit">保存绑定</button></div><p id="binding-status" role="status"></p><button type="button" id="binding-retry" hidden>重新读取已保存配置</button><div id="binding-conflict" hidden><details><summary>查看暂存的旧草稿</summary><pre id="binding-old-draft"></pre></details><button type="button" id="binding-keep-host">保留最新配置，放弃旧草稿</button><button type="button" id="binding-apply-draft">将旧草稿应用到当前配置（尚未保存）</button></div></form>';

 const form=content.querySelector('form'),status=content.querySelector('#binding-status'),button=form.querySelector('[type=submit]'),remove=content.querySelector('#binding-remove'),retry=content.querySelector('#binding-retry'),conflict=content.querySelector('#binding-conflict'),oldDraft=content.querySelector('#binding-old-draft'),apply=content.querySelector('#binding-apply-draft'),keep=content.querySelector('#binding-keep-host');
 const draft=options.draft||{},keys=['connection','model','size','quality','format'];
 const copy=v=>JSON.parse(JSON.stringify(v));
 const canonical=v=>JSON.stringify(v&&typeof v==='object'&&!Array.isArray(v)?Object.fromEntries(Object.keys(v).sort().map(k=>[k,JSON.parse(canonical(v[k]))])):v);
 const same=(a,b)=>canonical(a)===canonical(b);
 const baseline=card=>({imageBinding:card.imageBinding||null,imageParameters:card.imageParameters||{}});
 const values=()=>Object.fromEntries(keys.map(k=>[k,form.elements[k].value]));
 const fill=v=>{for(const k of keys)form.elements[k].value=v[k]||'';};
 let alive=true,sequence=0,busy=false,loaded=false,current=null,base=null,saved=null,connections=[],cards=[],review=false,confirming=false;
 function controls(){button.disabled=!alive||busy||review||!loaded||!current||!connections.some(row=>row.id===form.elements.connection.value&&row.status==='ready');remove.disabled=!alive||busy||review||!loaded||confirming||!current?.imageBinding;form.elements.preset.disabled=busy||!!options.presetId;apply.disabled=busy||!loaded;keep.disabled=busy||!loaded;retry.disabled=busy;for(const key of keys)form.elements[key].disabled=busy||!loaded||review;}
 function capture(){if(loaded&&current&&!review){if(same(values(),saved)&&!draft.uncertain){delete draft.values;delete draft.baseline;}else{draft.values=copy(values());draft.baseline=copy(base);}}}
 function clearDraft(){delete draft.values;delete draft.baseline;delete draft.uncertain;delete draft.attempt;delete draft.pending;review=false;conflict.hidden=true;}
 if(options.onDispose)options.onDispose(()=>{capture();alive=false;sequence++;});
 content.querySelector('#binding-back').onclick=()=>{capture();alive=false;sequence++;window.quickgenGatewaySettings.mount(content,invoke,fail).catch(fail);};
 const add=(select,value,label)=>{const option=document.createElement('option');option.value=value;option.textContent=label;select.append(option);};
 function showReview(message){review=true;status.textContent=message;conflict.hidden=false;oldDraft.textContent=[['connection','连接'],['model','模型'],['size','尺寸'],['quality','质量'],['format','返回方式']].map(([key,label])=>label+'：'+(draft.values?.[key]||'（未填写）')).join('\n');controls();}
 keep.onclick=()=>{if(!alive||!loaded||busy)return;clearDraft();status.textContent='已保留宿主最新配置，旧草稿已放弃。';controls();};
 apply.onclick=()=>{if(!alive||!loaded||busy)return;fill(draft.values||{});draft.baseline=copy(base);delete draft.uncertain;delete draft.attempt;delete draft.pending;review=false;conflict.hidden=true;status.textContent='旧草稿已重新应用，尚未保存；保存时仍会检查配置冲突。';controls();};
 async function show(){
  const ticket=++sequence;current=cards.find(card=>card.id===form.elements.preset.value)||null;loaded=false;review=false;conflict.hidden=true;fill({connection:connections.find(c=>c.status==='ready')?.id});status.textContent='正在读取已保存配置…';controls();
  if(!current){status.textContent=options.presetId?'此预设已不存在，不能保存。':'请先添加面板预设。';return;}
  const card=current;
  try{
   base=copy(baseline(card));
   if(card.imageBinding){const binding=await invoke('gateway_image_binding',{reference:card.imageBinding});if(!alive||ticket!==sequence)return;fill({connection:binding.capability.reference.connection_id,model:binding.policy.model,size:card.imageParameters?.size,quality:card.imageParameters?.quality,format:binding.policy.response_format});}
   if(!alive||ticket!==sequence)return;
   loaded=true;saved=copy(values());retry.hidden=true;
   status.textContent=card.imageBinding?(connections.some(row=>row.id===form.elements.connection.value&&row.status==='ready')?'已配置真实模型；实际调用仍须通过费用与额度检查。':'原连接未验证或已撤销，可选择其他连接或解除绑定。'):'未绑定模型；当前使用模拟执行。';
   if(draft.uncertain&&draft.attempt&&((draft.attempt.kind==='unbind'&&!card.imageBinding)||(draft.attempt.kind==='bind'&&same(saved,draft.attempt.values)))){clearDraft();status.textContent='已核对宿主：当前保存值与上次操作一致。';}
   else if(draft.values){
    if(draft.uncertain||!same(draft.baseline,base))showReview(draft.uncertain?'上次操作结果未确认；已重新读取最新保存值。旧草稿已暂存，请核对后明确选择是否重新应用。':'配置冲突：已显示最新保存值。旧草稿已暂存，只有明确选择才会重新应用。');
    else fill(draft.values);
   }
  }catch(error){if(alive&&ticket===sequence){loaded=false;status.textContent='已保存配置读取失败，禁止提交；请重试读取。旧草稿仍保留。';retry.hidden=false;fail(error);}}
  if(alive&&ticket===sequence)controls();
 }
 async function reload(){
  loaded=false;busy=true;controls();retry.hidden=true;status.textContent=draft.pending?'正在等待上次保存结束并核对宿主…':'正在读取已保存配置…';
  try{
   // Reconcile through the host queue, not a possibly lost reply from the old page.
   if(!alive)return;
   // The connection query uses the same FIFO host worker as bind/unbind.
   // Read configuration only after earlier queued mutations, even in a fresh page.
   const rows=await invoke('gateway_connections');if(!alive)return;
   const config=await invoke('load_config');if(!alive)return;
   cards=(config.abilities||[]).filter(card=>!options.presetId||card.id===options.presetId);connections=rows;
   form.elements.preset.innerHTML='';form.elements.connection.innerHTML='';
   for(const card of cards)add(form.elements.preset,card.id,card.name+(card.imageBinding?' · 已绑定':''));
   for(const c of connections)add(form.elements.connection,c.id,(c.account_id?c.account_id+' · '+c.base_url:c.id)+(c.status==='ready'?'':' · 未验证或已撤销'));
   form.elements.preset.value=options.presetId||cards[0]?.id||'';await show();
  }catch(error){if(alive){status.textContent='已保存配置读取失败，禁止提交；请重试读取。旧草稿仍保留。';retry.hidden=false;fail(error);}}
  finally{busy=false;if(alive)controls();}
 }
 retry.onclick=reload;
 form.elements.preset.onchange=()=>{clearDraft();return show();};form.elements.connection.onchange=controls;
 async function mutate(kind,args){
  draft.values=copy(values());draft.baseline=copy(base);draft.uncertain=true;draft.attempt={kind,values:copy(values())};busy=true;controls();status.textContent='正在保存…';
  const operation=(async()=>{
   try{
    const result=await invoke(kind==='bind'?'gateway_bind_preset':'gateway_unbind_preset',args);
    if(!alive)return;
    if(kind==='bind'){current.imageBinding=result.capability.reference;current.imageParameters=args.parameters;}else{delete current.imageBinding;delete current.imageParameters;fill({connection:connections.find(c=>c.status==='ready')?.id});}
    base=copy(baseline(current));saved=copy(values());clearDraft();status.textContent=kind==='bind'?'模型绑定已保存；未调用模型。':'已解除模型绑定，预设恢复模拟执行；已有任务与结果保留。';
   }catch(error){if(alive){loaded=false;status.textContent='操作结果未确认或配置冲突，请重新读取宿主状态后核对。旧草稿已保留。';retry.hidden=false;fail(error);}}
   finally{busy=false;if(alive)controls();}
  })();
  draft.pending=operation;await operation;if(draft.pending===operation)delete draft.pending;
 }
 remove.onclick=async()=>{
  if(remove.disabled||!alive)return;if(options.confirmRemove){confirming=true;controls();const accepted=await options.confirmRemove();confirming=false;controls();if(!accepted||!alive)return;}
  return mutate('unbind',{presetId:current.id,expected:copy(base)});
 };
 form.onsubmit=async event=>{
  event.preventDefault();if(button.disabled||!alive)return;
  const model=form.elements.model.value.trim(),size=form.elements.size.value.trim(),quality=form.elements.quality.value.trim();const parameters={};if(size)parameters.size=size;if(quality)parameters.quality=quality;
  form.elements.model.value=model;form.elements.size.value=size;form.elements.quality.value=quality;
  return mutate('bind',{connectionId:form.elements.connection.value,presetId:current.id,expected:copy(base),policy:{model,sizes:size?[size]:[],qualities:quality?[quality]:[],response_format:form.elements.format.value||null},parameters});
 };
 controls();await reload();
 if(options.presetId&&alive){
  form.elements.preset.closest('label').hidden=true;content.querySelector('#binding-back').hidden=true;
  const name=document.createElement('h2');name.textContent=current?.name||'预设不存在';form.prepend(name);
  const parameters=document.createElement('fieldset'),legend=document.createElement('legend');legend.textContent='生成参数';parameters.append(legend);for(const key of ['size','quality'])parameters.append(form.elements[key].closest('label'));form.insertBefore(parameters,form.querySelector('.actions'));
  const advanced=document.createElement('details'),summary=document.createElement('summary');summary.textContent='高级选项';advanced.append(summary,form.elements.format.closest('label'));form.insertBefore(advanced,form.querySelector('.actions'));
 }
}};
