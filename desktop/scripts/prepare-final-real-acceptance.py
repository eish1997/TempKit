"""Prepare one explicitly authorized CNY1.50 isolated acceptance store. No HTTP.
The original store is read-only; secrets are reprotected for the new DPAPI path.
"""
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import sqlite3
import time

ROOT=Path(__file__).resolve().parents[2]
SOURCE=ROOT/'desktop/evidence/generation/g0/native-connected-e211d75a0bde4190b83eafad5c5599e5'
OUT=ROOT/'desktop/evidence/generation/g4/final-real-acceptance'

def main():
    assert not OUT.exists(), 'Never replenish or overwrite an acceptance store'
    source_db=SOURCE/'resource-store/metadata.sqlite'
    before=hashlib.sha256(source_db.read_bytes()).hexdigest()
    src=sqlite3.connect(source_db.as_uri()+'?mode=ro',uri=True)
    config=json.loads((SOURCE/'config.json').read_text(encoding='utf-8-sig'))
    card=next(c for c in config['abilities'] if c['id']=='clay')
    assert card['imageParameters']=={'size':'1024x1024','quality':'low'}
    ref=card['imageBinding']
    raw,revoked=src.execute('SELECT connection_json,revoked FROM gateway_connections WHERE connection_id=?',(ref['connection_id'],)).fetchone()
    connection=json.loads(raw);assert not revoked
    profile=json.loads(src.execute('SELECT profile_json FROM gateway_profiles WHERE profile_id=?',(connection['profile_id'],)).fetchone()[0])
    assert profile['base_url'].rstrip('/')=='http://127.0.0.1:3000'
    binding=json.loads(src.execute('SELECT binding_json FROM image_bindings WHERE connection_id=? AND capability_id=? AND revision=?',(ref['connection_id'],ref['capability_id'],ref['revision'])).fetchone()[0])
    assert binding['policy']['model']=='gpt-image-2'
    originals={}
    for row in src.execute('SELECT * FROM resources'):
        metadata=json.loads(row[2])
        if metadata['display_name'] in ['clay-block-a.png','clay-block-b.png'] and metadata['resource']['provenance']['task_id'] is None:
            prior=originals.get(metadata['display_name'])
            assert prior is None or prior[1]['digest']==metadata['digest'], 'Conflicting test inputs'
            originals[metadata['display_name']]=(row,metadata)
    selected=[originals[name] for name in sorted(originals)]
    assert len(selected)==2, 'Exactly two original test images required'
    OUT.mkdir();store=OUT/'resource-store';store.mkdir()
    for directory in ['blobs','staging','credentials']:(store/directory).mkdir()
    (OUT/'quickgen-isolated-v1').write_bytes(b'QuickGen isolated test data v1')
    (OUT/'config.json').write_text(json.dumps(config,ensure_ascii=False),encoding='utf-8')
    if (SOURCE/'covers').exists():shutil.copytree(SOURCE/'covers',OUT/'covers')
    dst=sqlite3.connect(store/'metadata.sqlite')
    dst.executescript('PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL; BEGIN IMMEDIATE;\n'+
                      '\n'.join((ROOT/f'desktop/src-tauri/src/store_v{i}.sql').read_text(encoding='utf-8') for i in range(1,15))+'\nCOMMIT;')
    for table in ['gateway_profiles','gateway_connections','gateway_credential_rotations','image_bindings']:
        for row in src.execute('SELECT * FROM '+table):
            dst.execute('INSERT INTO '+table+' VALUES('+','.join('?' for _ in row)+')',row)
    # No copied authentication check: the new instance must probe again.
    dst.execute('INSERT INTO image_budget VALUES(1,1500000)')
    inputs=OUT/'test-inputs';inputs.mkdir()
    for i,(row,metadata) in enumerate(selected):
        blob=src.execute('SELECT * FROM blobs WHERE blob_key=?',(row[1],)).fetchone()
        image=(SOURCE/'resource-store/blobs'/row[1]).read_bytes()
        assert hashlib.sha256(image).hexdigest()==metadata['digest']==blob[1]
        (inputs/metadata['display_name']).write_bytes(image)
    # The app must activate its own empty scene and import through the real UI.
    # Never fabricate an active migration to prepopulate the board.
    dst.commit()
    spec=importlib.util.spec_from_file_location('vault_helper',ROOT/'desktop/scripts/probe-real-image-once.py')
    helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)  # run() is guarded; no HTTP
    references={connection['credential_ref']}
    references.update(row[0] for row in src.execute('SELECT credential_ref FROM gateway_credential_rotations WHERE connection_id=?',(connection['id'],)))
    for reference in references:
        key=json.dumps([profile['connection_id'],profile['profile_id'],profile['account_id'],reference],ensure_ascii=False,separators=(',',':'))
        name=hashlib.sha256(key.encode()).hexdigest()+'.dpapi'
        def entropy(vault):return ('QuickGen credential v1|'+('\\\\?\\'+str(vault))+'|'+key).encode()
        token=helper.crypt((SOURCE/'resource-store/credentials'/name).read_bytes(),entropy(SOURCE/'resource-store/credentials'))
        protected=helper.crypt(token,entropy(store/'credentials'),True)
        (store/'credentials'/name).write_bytes(protected)
        assert helper.crypt(protected,entropy(store/'credentials'))==token
        del token,protected
    assert dst.execute('PRAGMA integrity_check').fetchone()[0]=='ok' and not dst.execute('PRAGMA foreign_key_check').fetchall()
    assert dst.execute('SELECT count(*) FROM tasks').fetchone()[0]==0
    dst.close();src.close()
    assert hashlib.sha256(source_db.read_bytes()).hexdigest()==before
    report=dict(root=str(OUT),authorizedAdditionalCNY=1.5,cumulativeAuthorizationCNY=3.5,maximumNewRequests=3,perRequestReservationCNY=.5,
                sourceDatabaseUnchanged=True,resources=0,testInputs=2,tasks=0,authentication='fresh probe required',networkRequests=0,
                originalRecordsPreserved=True,credentialStorage='DPAPI reprotected for isolated path; no plaintext files')
    (OUT.parent/'final-real-prepared.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
    print(json.dumps(report))

if __name__=='__main__':main()
