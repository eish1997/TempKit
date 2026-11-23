"""Authenticate the explicitly prepared final isolation store; GET models only."""
import hashlib
import importlib.util
import json
from pathlib import Path
import sqlite3
import urllib.request

root=Path(__file__).resolve().parents[2]
data=root/'desktop/evidence/generation/g4/final-real-acceptance'
assert (data/'quickgen-isolated-v1').read_bytes()==b'QuickGen isolated test data v1'
spec=importlib.util.spec_from_file_location('vault_helper',root/'desktop/scripts/probe-real-image-once.py')
helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
db=sqlite3.connect(data/'resource-store/metadata.sqlite')
assert db.execute('SELECT count(*) FROM tasks').fetchone()[0]==0
assert db.execute('SELECT limit_micros FROM image_budget').fetchone()[0]==1500000
connection=json.loads(db.execute('SELECT connection_json FROM gateway_connections WHERE revoked=0').fetchone()[0])
profile=json.loads(db.execute('SELECT profile_json FROM gateway_profiles WHERE profile_id=?',(connection['profile_id'],)).fetchone()[0])
assert profile['base_url'].rstrip('/')=='http://127.0.0.1:3000'
rotated=db.execute('SELECT credential_ref FROM gateway_credential_rotations WHERE connection_id=? ORDER BY revision DESC LIMIT 1',(connection['id'],)).fetchone()
reference=rotated[0] if rotated else connection['credential_ref']
key=json.dumps([profile['connection_id'],profile['profile_id'],profile['account_id'],reference],ensure_ascii=False,separators=(',',':'))
vault=data/'resource-store/credentials'
entropy=('QuickGen credential v1|'+('\\\\?\\'+str(vault))+'|'+key).encode()
token=helper.crypt((vault/(hashlib.sha256(key.encode()).hexdigest()+'.dpapi')).read_bytes(),entropy).decode('ascii')
request=urllib.request.Request(profile['base_url'].rstrip('/')+'/v1/models',headers={'Authorization':'Bearer '+token},method='GET')
with urllib.request.build_opener(helper.NoRedirect()).open(request,timeout=20) as response:
    raw=response.read(2*1024*1024+1);assert len(raw)<=2*1024*1024
del token,request
models=[x['id'] for x in json.loads(raw)['data'] if isinstance(x.get('id'),str)]
assert 'gpt-image-2' in models
db.execute('INSERT OR REPLACE INTO gateway_checks VALUES(?,?)',(connection['id'],json.dumps(models)));db.commit();db.close()
report={'authenticated':True,'modelAvailable':'gpt-image-2','method':'GET /v1/models','paidRequests':0,'budgetCNY':1.5,'root':str(data)}
(data.parent/'final-connection-probe.json').write_text(json.dumps(report,indent=2),encoding='utf8')
print(json.dumps(report))
