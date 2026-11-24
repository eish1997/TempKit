(function(root){'use strict';
function mount({entry,row,invoke,onToggle,onChange,onError,onSaved,onClear}){let session={active:false,epoch:0,revision:0,style:{color:'#ed6957',width:3}},busy=false;
 let clearing=false,clearSequence=1;const clearStream=crypto.randomUUID();
 const clear=async()=>{if(clearing)return;clearing=true;try{consume(await invoke('ink_clear',{stream:clearStream,sequence:clearSequence}));clearSequence++;}finally{clearing=false;}};
 const sync=()=>{entry.title=session.active?'放回画笔 · 右键清空全部':'拿起画笔 · 右键清空全部';entry.setAttribute('aria-label',session.active?'放回画笔，退出绘制':'拿起画笔，双击输入文字，中键拖动最近一笔或文字');entry.setAttribute('aria-pressed',String(session.active));entry.setAttribute('aria-expanded',String(session.active));row.hidden=!session.active;for(const button of row.querySelectorAll('button'))button.setAttribute('aria-pressed',String(button.dataset.color?button.dataset.color===session.style.color:Number(button.dataset.width)===session.style.width));onChange();};
 const consume=data=>{if(!data?.session||data.session.epoch<session.epoch||data.session.revision<session.revision)return;session=data.session;sync();};
 const save=async next=>{if(busy)return;busy=true;try{consume(await invoke('ink_style',{style:next}));onSaved?.();}catch(error){onError(error);}finally{busy=false;}};
 for(const [color,label]of [['#ed6957','珊瑚红'],['#f4c85b','暖黄'],['#79bd91','草绿'],['#70aee4','天蓝'],['#b099db','淡紫'],['#faf6e9','米白'],['#484744','墨灰']]){const b=document.createElement('button');b.type='button';b.dataset.color=color;b.className='ink-swatch';b.style.setProperty('--swatch',color);b.setAttribute('aria-label',label);b.onclick=()=>save({...session.style,color});row.append(b);}for(const [width,label]of [[2,'细'],[3,'中'],[6,'粗']]){const b=document.createElement('button');b.type='button';b.dataset.width=width;b.setAttribute('aria-label',label+'笔画');const i=document.createElement('i');i.style.height=width+'px';b.append(i);b.onclick=()=>save({...session.style,width});row.append(b);}
 entry.title='拿起画笔 · 右键清空全部';entry.oncontextmenu=e=>{e.preventDefault();e.stopPropagation();onClear?.();};
 entry.onclick=()=>onToggle(!session.active);return{consume,clear,get active(){return session.active;},async set(value){try{consume(await invoke('ink_set_active',{enabled:value}));}finally{consume(await invoke('ink_snapshot'));}},async load(){consume(await invoke('ink_snapshot'));}};
}
root.quickgenInkTools={mount};
})(window);
