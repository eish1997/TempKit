const invoke=window.__TAURI__.core.invoke;
const presets=[['clay','白模转写实','将白模场景转成写实效果'],['material','材质替换','修改物体材质与表面属性'],['character','角色精修','优化细节、质感与外观'],['mood','场景氛围','调整灯光、天气与环境'],['expand','图片扩展','扩展画面边界与构图'],['style','风格迁移','转换视觉风格']].map(([id,name,prompt])=>({id,name,prompt,cover:''}));
const content=document.getElementById('content'),message=document.getElementById('error');
const fail=e=>message.textContent=String(e);document.getElementById('dismiss').innerHTML=quickgenIcons.x;document.getElementById('dismiss').onclick=()=>invoke('close_manager').catch(fail);
let abilities=[],cover='';
// The host owns atomic append and duplicate detection, including across reloads.
function save(ability){return invoke('add_ability',{ability})}
function libraryPage(){return {title:'预设库',mount:async ctx=>{
 const cfg=await ctx.invoke('load_config');abilities=cfg.abilities;
 const items=[...presets,...abilities.filter(a=>!presets.some(p=>p.id===a.id))];
 for(const item of items){const row=document.createElement('div');row.className='row';const info=document.createElement('div'),title=document.createElement('strong'),desc=document.createElement('p');title.textContent=item.name;desc.textContent=item.prompt;info.append(title,desc);row.append(info);
 const added=abilities.some(a=>a.id===item.id),button=document.createElement('button');button.id='preset-'+item.id;button.textContent=added?'模型设置':'添加';
 button.onclick=async()=>{if(added)return ctx.push(window.quickgenPresetSettingsPage(item.id));button.disabled=true;try{await ctx.invoke('add_ability',{ability:item});button.textContent='模型设置';button.disabled=false;button.onclick=()=>ctx.push(window.quickgenPresetSettingsPage(item.id));}catch(error){ctx.fail(error);button.disabled=false;}};row.append(button);ctx.root.append(row);
 }
}};}
const mode=new URLSearchParams(location.search).get('mode');document.getElementById('heading').textContent=mode==='library'?'预设库':'自定义能力';
if(['settings','gateway','library','binding'].includes(mode)){
 const back=document.getElementById('back');back.innerHTML=quickgenIcons['arrow-left'];
 const navigation=window.quickgenManagerNavigation.create({content,heading:document.getElementById('heading'),back,message,invoke,onRootBack:mode==='binding'?()=>document.getElementById('dismiss').onclick():null});
 document.getElementById('dismiss').onclick=async()=>{try{await invoke('close_manager');navigation.dispose();}catch(error){fail(error);}};
 navigation.start(mode==='binding'?window.quickgenPresetSettingsPage(new URLSearchParams(location.search).get('presetId')):mode==='library'?libraryPage():mode==='gateway'?window.quickgenSettingsPages.connections():window.quickgenSettingsPages.home());
}else{
content.innerHTML='<form><label>能力名称<input name="name" required maxlength="24"></label><label>处理指令<textarea name="prompt" required maxlength="2000"></textarea></label><label>卡片封面<input name="cover" type="file" accept="image/png,image/jpeg,image/webp"></label><div class="actions"><button type="button" id="cancel">取消</button><button type="submit">添加到面板</button></div></form>';
const form=content.querySelector('form');document.getElementById('cancel').onclick=()=>invoke('close_manager').catch(fail);
let coverTask=Promise.resolve(),coverError=null,coverSequence=0,saved=false;
form.elements.cover.onchange=e=>{
 const sequence=++coverSequence,file=e.target.files[0];cover='';coverError=null;message.textContent='';
 coverTask=(async()=>{
  if(!file)return;
  if(!['image/png','image/jpeg','image/webp'].includes(file.type)||file.size>5*1024*1024)throw Error('请选择 5 MB 以内的 PNG/JPG/WebP');
  const data=await new Promise((resolve,reject)=>{const r=new FileReader();r.onload=()=>resolve(r.result);r.onerror=()=>reject(Error('封面读取失败'));r.readAsDataURL(file)});
  const img=new Image();img.src=data;await img.decode();
  if(sequence===coverSequence)cover=data;
 })().catch(e=>{if(sequence===coverSequence){coverError=e instanceof DOMException?Error('封面图片损坏或格式不支持'):e;fail(coverError)}});
};
form.onsubmit=async e=>{
 e.preventDefault();const button=form.querySelector('[type=submit]');if(button.disabled)return;
 button.disabled=true;form.elements.cover.disabled=true;
 try{if(!saved){await coverTask;if(coverError)throw coverError;const name=form.elements.name.value.trim(),prompt=form.elements.prompt.value.trim();if(!name||!prompt)throw Error('名称与处理指令不能为空');await save({id:crypto.randomUUID(),name,prompt,cover});saved=true;form.elements.name.disabled=true;form.elements.prompt.disabled=true;button.textContent='关闭'}await invoke('close_manager')}
 catch(e){fail(e);button.disabled=false;form.elements.cover.disabled=saved}
};
}
