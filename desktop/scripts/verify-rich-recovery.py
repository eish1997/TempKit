"""Copies and audits synthetic M-stage stores only; never opens production data."""
import hashlib,json,pathlib,shutil,sqlite3,sys,uuid
BASE=pathlib.Path(__file__).resolve().parents[1]/'evidence'/'notes-rich-migration'
def bounded(value):
    if str(value).startswith('\\\\?\\'): value=str(value)[4:]
    p=pathlib.Path(value).resolve()
    if not p.is_relative_to(BASE.resolve()): raise ValueError('outside M evidence')
    return p
def read(p): return json.loads(p.read_text(encoding='utf-8-sig'))
def write(p,v): p.write_text(json.dumps(v,ensure_ascii=False,indent=2),encoding='utf-8')
def notes(root):
    with sqlite3.connect((root/'resource-store'/'metadata.sqlite').as_uri()+'?mode=ro',uri=True) as c:
        return sorted((json.loads(x[0]) for x in c.execute('select note_json from notes')),key=lambda n:n['id'])
def table_data(root):
    with sqlite3.connect((root/'resource-store'/'metadata.sqlite').as_uri()+'?mode=ro',uri=True) as c:
        return {name:sorted([list(row) for row in c.execute('SELECT * FROM "'+name+'"')],key=repr) for name, in c.execute("SELECT name FROM sqlite_master WHERE type='table'") if name not in ['notes','note_tag_births','schema_migrations','sqlite_sequence']}
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
action=sys.argv[1]
if action=='snapshot':
    root=bounded(sys.argv[2]);database=root/'resource-store'/'metadata.sqlite'
    with sqlite3.connect(database.as_uri()+'?mode=ro',uri=True) as src, sqlite3.connect(root/'confirmed-before-crash.sqlite') as dst:src.backup(dst)
    write(root/'expected-recovery.json',{'mode':'crash','notes':notes(root),'otherTables':table_data(root)})
    print('Confirmed committed snapshot retained')
elif action in ['prepare','legacy']:
    source=bounded(sys.argv[2]);result=read(source/'board-smoke-result.json');assert result['ok']
    folder=bounded(result['exportDirectory']);manifest=read(folder/'manifest.json')
    for entry in manifest['files']:
        item=(folder/entry['path']).resolve();assert item.is_relative_to(folder);assert digest(item)==entry['sha256'].lower(),entry['path']
    root=BASE/('rich-'+action+'-'+uuid.uuid4().hex);root.mkdir();(root/'quickgen-isolated-v1').write_text('QuickGen isolated test data v1',encoding='ascii')
    shutil.copytree(folder,root/'resource-store');original=notes(root)
    if action=='legacy':
        fixtures=read(BASE/'legacy-note-fixtures.json')
        for n,fixture in zip(original[-len(fixtures):],fixtures):
            n['text']=fixture['text'];n['deleted']=fixture.get('deleted',False);n['resident']=False
        with sqlite3.connect(root/'resource-store'/'metadata.sqlite') as c:
            c.execute('drop table note_tag_births');c.execute('delete from schema_migrations where version=15');c.execute('pragma user_version=14')
            for n in original:
                for key in ['document','theme','theme_revision']:n.pop(key)
                c.execute('update notes set note_json=? where note_id=?',(json.dumps(n,ensure_ascii=False),n['id']))
        # Retain an immutable synthetic pre-upgrade copy for rollback comparison.
        shutil.copy2(root/'resource-store'/'metadata.sqlite',root/'schema14-before.sqlite')
    write(root/'expected-recovery.json',{'mode':action,'notes':original,'otherTables':table_data(root),'export':str(folder),'source':str(source)})
    print(root)
elif action in ['audit','audit-default']:
    root=bounded(sys.argv[2]);expected=read(root/'expected-recovery.json')
    if action=='audit-default':
        result=read(pathlib.Path(str(root)+'.identity.json'));assert result['mode']=='default-launch' and result['exitCode']==0
        assert len([w for w in result['nativeWindows'] if w['title']=='便签' and w['visible']])==10
    else:
        result=read(root/'board-smoke-result.json');assert result['ok'],result.get('error')
    after=notes(root);assert len(after)==len(expected['notes'])
    with sqlite3.connect(root/'resource-store'/'metadata.sqlite') as c:assert c.execute('pragma user_version').fetchone()[0]==15
    for old,new in zip(expected['notes'],after):
        if expected['mode']=='legacy':
            assert new['theme']=='default' and new['theme_revision']==0
            paragraphs=new['document']['doc']['content'];assert all(p['type']=='paragraph' for p in paragraphs)
            assert '\n'.join(''.join(t['text'] for t in p.get('content',[])) for p in paragraphs)==old['text']
        for key in old:
            if key in ['revision','geometry_revision','updated_at','monitor']:continue
            assert new[key]==old[key],(old['id'],key)
    assert table_data(root)==expected['otherTables'],'unrelated tables changed'
    audit={'ok':True,'notes':len(after),'mode':expected['mode'],'defaultBuild':action=='audit-default','richDocumentThemeFlagsAndGeometryEqual':True,'unrelatedTablesEqual':True,'nativeRestartResult':str(root)+'.identity.json' if action=='audit-default' else str(root/'board-smoke-result.json')}
    write(root/'recovery-audit.json',audit);print(json.dumps(audit))
else: raise ValueError(action)
