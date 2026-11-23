"""Read-only cutover verification against an accepted rehearsal; no UI automation."""
import argparse
import base64
import hashlib
import json
import sqlite3
from pathlib import Path

parser=argparse.ArgumentParser()
parser.add_argument('--profile',required=True)
parser.add_argument('--reference',required=True)
parser.add_argument('--output',required=True)
args=parser.parse_args()

def load(profile):
    profile=Path(profile).resolve();root=profile/'resource-store'
    connection=sqlite3.connect((root/'metadata.sqlite').as_uri()+'?mode=ro',uri=True)
    connection.execute('PRAGMA query_only=ON')
    assert connection.execute('PRAGMA integrity_check').fetchone()[0]=='ok','SQLite integrity failed'
    assert not connection.execute('PRAGMA foreign_key_check').fetchall(),'Foreign keys failed'
    runs=connection.execute("SELECT migration_id,source_digest,snapshot_json FROM migration_runs WHERE state='active'").fetchall()
    assert len(runs)==1,'Exactly one active migration required'
    run,digest,source=runs[0];source_bytes=source.encode('utf-8');assert hashlib.sha256(source_bytes).hexdigest()==digest
    assert (root/'migration-backups'/f'{digest}.json').read_bytes()==source_bytes,'Source backup changed'
    mappings=dict(connection.execute('SELECT legacy_key,resource_id FROM migration_id_maps WHERE migration_id=?',(run,)))
    records={}
    for resource,key,metadata,state,body_digest,length in connection.execute('SELECT r.resource_id,r.blob_key,r.metadata_json,r.state,b.digest,b.byte_length FROM resources r JOIN blobs b ON b.blob_key=r.blob_key'):
        path=(root/'blobs'/key).resolve();assert path.parent==(root/'blobs').resolve(),'Blob escaped managed directory'
        body=path.read_bytes();assert len(body)==length and hashlib.sha256(body).hexdigest()==body_digest,'Resource body differs'
        record=json.loads(metadata);assert record['resource']['id']==resource and record['resource']['blob_id']==key
        assert state=='available','Initial migrated resource unavailable';records[resource]=body
    snapshot=json.loads(source)
    original={f"item:{i['id']}":i['src'] for i in snapshot['items']}
    original.update({f"job:{j['id']}:input":j['input']['src'] for j in snapshot['jobs']})
    assert set(original)==set(mappings),'Legacy mapping set differs'
    for key,url in original.items():assert records[mappings[key]]==base64.b64decode(url.split(',',1)[1],validate=True),'Original image bytes changed'
    scene=json.loads(connection.execute("SELECT scene_json FROM scenes WHERE scene_id='main'").fetchone()[0])
    inverse={v:k for k,v in mappings.items()}
    def normalize(value):
        if isinstance(value,str):return inverse.get(value,value)
        if isinstance(value,list):return [normalize(x) for x in value]
        if isinstance(value,dict):return {k:normalize(v) for k,v in value.items()}
        return value
    tasks=connection.execute('SELECT count(*) FROM tasks').fetchone()[0];connection.close()
    backup=snapshot['configBackup'];config=profile/'config.json'
    if config.exists():assert json.loads(config.read_text(encoding='utf-8-sig'))==backup['config'],'Config differs from migrated source'
    for name,encoded in backup['covers'].items():
        path=(profile/'covers'/name).resolve();assert path.parent==(profile/'covers').resolve(),'Cover escaped directory'
        assert path.read_bytes()==base64.b64decode(encoded,validate=True),'Cover bytes changed'
    comparable=scene.copy();comparable.pop('revision',None)
    return {'scene':normalize(comparable),'source':snapshot,'counts':{'resources':len(records),'mappings':len(mappings),'items':len(scene['items']),'historicalJobs':len(scene['jobs']),'durableTasks':tasks},'sourceDigest':digest}

actual=load(args.profile);reference=load(args.reference)
assert actual['source']==reference['source'],'Original scene/config backup differs from accepted rehearsal'
assert actual['scene']==reference['scene'],'Normalized migrated scene differs from accepted rehearsal'
assert actual['counts']==reference['counts'],'Entity counts differ from accepted rehearsal'
report={'ok':True,'profile':str(Path(args.profile).resolve()),'reference':str(Path(args.reference).resolve()),'counts':actual['counts'],'sourceDigest':actual['sourceDigest'],'checks':['SQLite integrity/foreign keys','one active migration','exact original backup','unique legacy mapping inventory','every original image byte','all normalized scene fields','config and cover contents']}
Path(args.output).write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps(report,ensure_ascii=False))
