(async()=>{
 const invoke=window.__TAURI__?.core.invoke;if(!invoke||!await invoke('board_diagnostic'))return;
 let previous=0,busy=false;
 setInterval(async()=>{if(busy)return;busy=true;const start=performance.now();try{await invoke('board_probe',{elapsed:previous});previous=performance.now()-start}finally{busy=false}},1000);
})().catch(console.error);
