(function(root){
 'use strict';
 function persist(db,snapshot){
  const fixed=structuredClone(snapshot);
  return new Promise((resolve,reject)=>{
   let tx;try{tx=db.transaction('scene','readwrite');tx.objectStore('scene').put(fixed,'current')}catch(error){reject(error);return}
   tx.oncomplete=()=>resolve(fixed);
   tx.onerror=()=>reject(tx.error||Error('Snapshot write failed'));
   tx.onabort=()=>reject(tx.error||Error('Snapshot write aborted'));
  });
 }
 async function prepare(db,snapshot,invoke){const fixed=await persist(db,snapshot);return invoke('migration_prepare',{snapshot:fixed})}
 const api={persist,prepare};if(typeof module==='object')module.exports=api;else root.quickgenLegacySnapshot=api;
})(globalThis);
