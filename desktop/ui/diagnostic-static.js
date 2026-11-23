(async()=>{
 const invoke=window.__TAURI__.core.invoke;
 const db=await new Promise((resolve,reject)=>{const r=indexedDB.open('quickgen-board-single-v1',1);r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(r.error)});
 const scene=await new Promise((resolve,reject)=>{const r=db.transaction('scene').objectStore('scene').get('current');r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(r.error)});db.close();
 if(!scene?.items.length)throw Error('No existing images for diagnostic');
 const world=document.getElementById('world');world.style.transform=`translate(${scene.tx}px,${scene.ty}px) scale(${scene.scale})`;
 for(const item of scene.items){const el=document.createElement('div');el.className='image';el.style.left=item.x+'px';el.style.top=item.y+'px';const img=document.createElement('img');img.src=item.src;const label=document.createElement('span');label.textContent=item.name;el.append(img,label);world.append(el)}
 await Promise.all([...document.images].map(img=>img.decode()));
 let pass=true;
 async function sync(){const visuals=[...world.children].map(el=>{const r=el.getBoundingClientRect();return{x:r.x,y:r.y,width:r.width,height:r.height}}).filter(r=>r.x<innerWidth&&r.y<innerHeight&&r.x+r.width>0&&r.y+r.height>0);await invoke('board_view',{view:{open:true,passthrough:pass,pinned:false,interacting:false,rects:[],visuals,panel:{x:0,y:0,width:0,height:0}}})}
 const channel=new window.__TAURI__.core.Channel();channel.onmessage=e=>{if(e.type==='toggle'){pass=!pass;sync()}if(e.type==='display')sync()};await invoke('board_subscribe',{channel});await sync();
})().catch(e=>{console.error(e);document.body.textContent=String(e)});
