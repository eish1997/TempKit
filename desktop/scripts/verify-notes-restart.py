"""Prepare a separate verified recovery copy, or audit a completed native restart."""
import argparse, hashlib, json, shutil, sqlite3, uuid
from pathlib import Path

def canonical(path):
    # Rust canonicalize uses Windows extended paths; normalize only that prefix
    # after resolving links, before applying the same containment check.
    value = str(Path(path).resolve())
    if value.startswith('\\\\?\\UNC\\'):
        value = '\\\\' + value[8:]
    elif value.startswith('\\\\?\\'):
        value = value[4:]
    return Path(value)


def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))

def hashes(export):
    manifest = read(export / 'manifest.json')
    for entry in manifest['files']:
        path = canonical(export / entry['path'])
        assert path.is_relative_to(export), 'Export entry escaped root'
        assert hashlib.sha256(path.read_bytes()).hexdigest().lower() == entry['sha256'].lower(), entry['path']
    return manifest

def prepare(identity_path):
    identity = read(identity_path)
    assert identity['status'] == 'exited' and identity['exitCode'] == 0 and identity['result']['ok']
    source = canonical(identity['root'])
    export = canonical(identity['result']['exportDirectory'])
    assert export.is_relative_to(source / 'resource-store' / 'recovery-exports')
    manifest = hashes(export)
    root = Path(__file__).resolve().parent.parent / 'evidence' / 'notes' / 'n4' / ('notes-restart-' + uuid.uuid4().hex)
    root.mkdir()
    (root / 'quickgen-isolated-v1').write_bytes(b'QuickGen isolated test data v1')
    shutil.copytree(export, root / 'resource-store')
    hashes(root / 'resource-store')
    plan = {'root': str(root), 'sourceIdentity': str(identity_path.resolve()), 'export': str(export), 'noteCount': manifest['noteCount']}
    (root / 'restart-plan.json').write_text(json.dumps(plan, indent=2), encoding='utf-8')
    return plan

def audit(root):
    plan = read(root / 'restart-plan.json')
    export = Path(plan['export'])
    hashes(export)  # The original backup must remain unchanged.
    expected = read(export / 'inventory.json')['notes']
    identity = read(root / 'restart-identity.json')
    assert identity['status'] == 'exited' and identity['exitCode'] == 0
    result = read(root / 'board-smoke-result.json')
    assert result['ok'] and result['kind'] == 'notes-restart'
    def content(notes):
        return sorted(({k: n[k] for k in ('id', 'text', 'resident', 'always_on_top', 'deleted')} for n in notes), key=lambda n: n['id'])
    assert content(expected) == content(result['before']) == content(result['after'])
    resident = [n for n in expected if n['resident'] and not n['deleted']]
    assert sorted(result['editors'], key=lambda n: n['id']) == sorted(({'id': n['id'], 'text': n['text']} for n in resident), key=lambda n: n['id'])
    with sqlite3.connect((root / 'resource-store' / 'metadata.sqlite').as_uri() + '?mode=ro', uri=True) as db:
        assert db.execute('pragma integrity_check').fetchone()[0] == 'ok'
        actual = [json.loads(row[0]) for row in db.execute('select note_json from notes')]
    assert content(actual) == content(expected)
    report = {'ok': True, 'candidate': identity['sha256'], 'noteCount': len(expected), 'nativeEditorsChecked': len(resident), 'backupHashesUnchanged': True, 'normalExit': True, 'physicalInputVerified': False, 'geometryNativeAcceptance': 'Separate gate; placement may be clamped to available monitors'}
    (root / 'restart-audit.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    return report

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('mode', choices=['prepare', 'audit'])
    parser.add_argument('path', type=Path)
    args = parser.parse_args()
    print(json.dumps(prepare(args.path) if args.mode == 'prepare' else audit(args.path.resolve())))
