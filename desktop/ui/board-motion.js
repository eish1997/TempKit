(function(root){
const mix=(a,b,t)=>Object.fromEntries(['x','y','width','height'].map(k=>[k,a[k]+(b[k]-a[k])*t]));
function ease(t){let lo=0,hi=1,u=t;for(let i=0;i<14;i++){u=(lo+hi)/2;const x=3*(1-u)*(1-u)*u*.23+3*(1-u)*u*u*.32+u*u*u;if(x<t)lo=u;else hi=u}return 1-Math.pow(1-u,3)}
function createGate(now=()=>performance.now()){let deadline=0;return{opening(){deadline=Infinity},expanded(){deadline=now()+1000},closed(){deadline=Infinity},allows(){return now()>=deadline}}}
function mount({dock,notch,prepare,changed}){
 const shell=document.createElement('div');shell.className='panel-morph-shell';shell.hidden=true;document.body.append(shell);
 const reduced=matchMedia('(prefers-reduced-motion: reduce)');let current=null,targetVisible=!dock.hidden,frame=0,sequence=0,active=false,resolvePending;
 const bounds=el=>{const r=el.getBoundingClientRect();return{x:r.x,y:r.y,width:r.width,height:r.height}};
 function draw(r,alpha){current=r;shell.style.clipPath=`inset(${r.y}px ${Math.max(0,innerWidth-r.x-r.width)}px ${Math.max(0,innerHeight-r.y-r.height)}px ${r.x}px round 6px)`;const p=bounds(dock);dock.style.clipPath=`inset(${Math.max(0,r.y-p.y)}px ${Math.max(0,p.x+p.width-r.x-r.width)}px ${Math.max(0,p.y+p.height-r.y-r.height)}px ${Math.max(0,r.x-p.x)}px round 6px)`;dock.style.opacity=String(alpha)}
 function finish(){active=false;dock.hidden=!targetVisible;notch.hidden=targetVisible;dock.inert=false;dock.style.clipPath='';dock.style.opacity='';dock.classList.remove('morphing');shell.hidden=true;current=null;changed?.();resolvePending?.(true);resolvePending=null}
 async function set(visible,instant=false){if(visible===targetVisible&&!active)return true;const id=++sequence;cancelAnimationFrame(frame);resolvePending?.(false);resolvePending=null;const wasVisible=targetVisible;targetVisible=visible;dock.hidden=false;notch.hidden=false;const panel=bounds(dock),entry=bounds(notch),from=current|| (wasVisible?panel:entry),to=visible?panel:entry;notch.hidden=true;
  if(instant||reduced.matches){finish();return true}
  active=true;dock.inert=true;dock.classList.add('morphing');shell.hidden=false;const priorAlpha=parseFloat(dock.style.opacity),alpha=Number.isFinite(priorAlpha)?priorAlpha:wasVisible?1:0;draw(from,alpha);
  await prepare?.();if(id!==sequence)return false;
  return new Promise(resolve=>{resolvePending=resolve;const start=performance.now();function tick(now){if(id!==sequence)return;const t=Math.min(1,(now-start)/240),p=ease(t);draw(mix(from,to,p),visible?alpha+(1-alpha)*Math.max(0,(p-.35)/.65):alpha*Math.max(0,1-p*1.8));if(t===1)finish();else frame=requestAnimationFrame(tick)}frame=requestAnimationFrame(tick)})
 }
 return{set,get active(){return active},get visible(){return targetVisible},settle(){if(active){++sequence;cancelAnimationFrame(frame);finish()}}};
}
const api={mix,ease,createGate,mount};if(typeof module==='object'&&module.exports)module.exports=api;else root.quickgenBoardMotion=api;
})(globalThis);
