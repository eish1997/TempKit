const invoke=window.__TAURI__.core.invoke;
const presets=[['clay','白模转写实','将白模场景转成写实效果'],['material','材质替换','修改物体材质与表面属性'],['character','角色精修','优化细节、质感与外观'],['mood','场景氛围','调整灯光、天气与环境'],['expand','图片扩展','扩展画面边界与构图'],['style','风格迁移','转换视觉风格']].map(([id,name,prompt])=>({id,name,prompt,cover:''}));
const content=document.getElementById('content'),message=document.getElementById('error');
const fail=e=>message.textContent=String(e);document.getElementById('dismiss').innerHTML=quickgenIcons.x;document.getElementById('dismiss').onclick=()=>invoke('close_manager').catch(fail);
let abilities=[],cover='';
// The host owns atomic append and duplicate detection, including across reloads.
function save(ability){return invoke('add_ability',{ability})}
let libraryRenderSequence=0;
async function library(){const sequence=++libraryRenderSequence;const cfg=await invoke('load_config');if(sequence!==libraryRenderSequence)return;abilities=cfg.abilities;content.replaceChildren();const settings=document.createElement('button');settings.type='button';settings.textContent='New API 连接';settings.onclick=async()=>{try{await invoke('open_manager',{mode:'gateway'});await invoke('show_manager');}catch(error){fail(error);}};content.append(settings);for(const item of presets){const row=document.createElement('div');row.className='row';const info=document.createElement('div');const title=document.createElement('strong');title.textContent=item.name;const desc=document.createElement('p');desc.textContent=item.prompt;info.append(title,desc);const add=document.createElement('button');const added=abilities.some(a=>a.id===item.id);add.innerHTML=quickgenIcons[added?'check':'plus']+'<span>'+(added?'已添加':'添加')+'</span>';add.disabled=abilities.some(a=>a.id===item.id);add.onclick=async()=>{add.disabled=true;try{await save(item);await library()}catch(e){fail(e);add.disabled=false}};row.append(info,add);content.append(row)}}
const mode=new URLSearchParams(location.search).get('mode');document.getElementById('heading').textContent=mode==='library'?'预设库':'自定义能力';
if(mode==='gateway'){document.getElementById('heading').textContent='New API 连接';quickgenGatewaySettings.mount(content,invoke,fail).catch(fail);}else if(mode==='library')library().catch(fail);else{
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
