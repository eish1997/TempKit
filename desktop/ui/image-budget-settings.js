'use strict';
window.quickgenImageBudget={async mount(content,invoke,fail,options={}){
 content.innerHTML='<p id="budget-status" role="status">正在读取调用额度…</p><p>这是 QuickGen 本地调用限额，不是 New API 余额或实际账单。按已核验的单次费用上限预留。模型费用尚未核验时不会发送请求；重试保存结果不会再次预留额度。</p><form><label>调用总额度（人民币元）<input name="amount" inputmode="decimal" required maxlength="20" placeholder="例如：2.00"></label><p>一经设置，当前版本不支持修改。保存额度本身不会调用模型。</p><div class="actions"><button type="button" id="budget-back">返回连接</button><button type="submit">保存总额度</button></div></form>';
 const form=content.querySelector('form'),status=content.querySelector('#budget-status'),button=form.querySelector('[type=submit]');let alive=true,busy=false,loaded=false,saved=false;
 const controls=()=>{button.disabled=busy||!loaded||saved;form.elements.amount.disabled=busy||!loaded||saved;};
 const yuan=value=>(value/1000000).toFixed(6).replace(/0+$/,'').replace(/\.$/,'');
 const paint=value=>{saved=value.limit_micros!==null;loaded=true;status.textContent=saved?'总额度 ¥'+yuan(value.limit_micros)+'，已预留 ¥'+yuan(value.reserved_micros)+'，可用 ¥'+yuan(Math.max(0,value.limit_micros-value.reserved_micros))+'。模型费用核验仍由宿主检查。':'尚未设置调用总额度，真实模型任务不会发送。';form.hidden=saved;if(saved)form.elements.amount.value=yuan(value.limit_micros);controls();};
 if(options.onDispose)options.onDispose(()=>{alive=false;});if(options.navigation)content.querySelector('#budget-back').hidden=true;
 content.querySelector('#budget-back').onclick=()=>{alive=false;window.quickgenGatewaySettings.mount(content,invoke,fail).catch(fail);};
 form.onsubmit=async event=>{
  event.preventDefault();if(!alive||button.disabled)return;
  const value=form.elements.amount.value.trim();
  if(!/^\d{1,7}(\.\d{1,6})?$/.test(value)){fail('请输入正数金额，最多六位小数。');return;}
  const [whole,fraction='']=value.split('.'),limitMicros=Number(whole)*1000000+Number(fraction.padEnd(6,'0'));
  if(!Number.isSafeInteger(limitMicros)||limitMicros<=0||limitMicros>1000000000000){fail('额度须大于 0 且不超过 100 万元。');return;}
  busy=true;controls();try{const result=await invoke('image_budget',{limitMicros});if(alive)paint(result);}catch(error){if(alive)fail(error);}finally{if(alive){busy=false;controls();}}
 };
 controls();try{const result=await invoke('image_budget',{});if(alive)paint(result);}catch(error){if(alive){status.textContent='额度读取失败，请返回后重试。';fail(error);}}
}};
