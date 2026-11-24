'use strict';
window.quickgenPresetSettingsPage=(presetId)=>{
 const draft={};
 return {title:'模型设置',mount:async ctx=>{
  if(typeof presetId!=='string'||!presetId)throw Error('缺少预设 ID，不能保存。');
  await window.quickgenImageBindingSettings.mount(ctx.root,ctx.invoke,ctx.fail,{presetId,draft,onDispose:ctx.onDispose,confirmRemove:()=>new Promise(resolve=>{
   const panel=document.createElement('div'),text=document.createElement('p'),yes=document.createElement('button'),no=document.createElement('button');
   text.textContent='解除后，后续使用模拟执行，已有任务与结果保留。';yes.textContent='确认解除';no.textContent='取消';panel.append(text,no,yes);ctx.root.append(panel);no.focus();
   const finish=value=>{panel.remove();resolve(value);};yes.onclick=()=>finish(true);no.onclick=()=>finish(false);ctx.onDispose(()=>finish(false));
  })});
  if(!ctx.active())return;
  const form=ctx.root.querySelector('form'),suggestions=document.createElement('datalist');suggestions.id='preset-model-suggestions';ctx.root.append(suggestions);form.elements.model.setAttribute('list',suggestions.id);
  const populate=()=>{suggestions.replaceChildren();const cached=window.quickgenConnectionPages.models.get(form.elements.connection.value);if(cached)for(const model of cached.items){const option=document.createElement('option');option.value=model;suggestions.append(option);}};
  const changed=form.elements.connection.onchange;form.elements.connection.onchange=()=>{changed();populate();};populate();
  const hint=document.createElement('p');hint.textContent='前往添加连接或额度页时保留本页草稿；返回预设库或关闭窗口会放弃未保存内容。';ctx.root.append(hint);
  for(const [id,title,page] of [['binding-add-connection','添加连接',window.quickgenConnectionPages.add()],['binding-connections','管理与检测连接',window.quickgenConnectionPages.list()],['binding-budget','查看调用额度',window.quickgenSettingsPages.budget()]]){
   const button=document.createElement('button');button.id=id;button.type='button';button.textContent=title;button.onclick=()=>ctx.push(page);ctx.root.append(button);
  }
 }};
};
