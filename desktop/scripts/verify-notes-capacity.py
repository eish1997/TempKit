"""Read-only capacity audit; never substitutes for a native restart or input test."""
import argparse, hashlib, json, sqlite3
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



def read_json(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))


def audit(identity_path):
    identity = read_json(identity_path)
    root = canonical(identity['root'])
    result = read_json(root / 'board-smoke-result.json')
    assert identity['status'] == 'exited' and identity['exitCode'] == 0, 'Supervisor has not confirmed a successful exit'
    assert result.get('ok') and result['elapsedMs'] >= 1800000, 'Full 30 minute run not complete'
    export = canonical(result['exportDirectory'])
    assert export.is_relative_to(root / 'resource-store' / 'recovery-exports'), 'Unexpected export location'
    manifest = read_json(export / 'manifest.json')
    assert manifest['databaseSchema'] == 7 and manifest['noteCount'] == 100
    for entry in manifest['files']:
        path = canonical(export / entry['path'])
        assert path.is_relative_to(export), 'Manifest path escaped export'
        assert hashlib.sha256(path.read_bytes()).hexdigest().lower() == entry['sha256'].lower(), entry['path']
    inventory = read_json(export / 'inventory.json')
    def notes_at(path):
        with sqlite3.connect(path.resolve().as_uri() + '?mode=ro', uri=True) as connection:
            assert connection.execute('pragma integrity_check').fetchone()[0] == 'ok'
            return sorted((json.loads(row[0]) for row in connection.execute('select note_json from notes')), key=lambda n: n['id'])
    source_notes = notes_at(root / 'resource-store' / 'metadata.sqlite')
    export_notes = notes_at(export / 'metadata.sqlite')
    assert source_notes == export_notes == sorted(inventory['notes'], key=lambda n: n['id']), 'Source/export/inventory differ'
    assert len(source_notes) == 100 and sum(n['resident'] for n in source_notes) == 10
    assert sum(n['text'] == '' for n in source_notes) == 1
    assert not any(n['deleted'] for n in source_notes)
    return {'ok': True, 'candidate': identity['sha256'], 'elapsedMs': result['elapsedMs'], 'edits': result['edits'], 'cycles': result['cycles'], 'notes': 100, 'resident': 10, 'blank': 1, 'sourceExportInventoryEqual': True, 'allExportHashesMatch': True, 'nativeRestartVerified': False, 'physicalInputVerified': False, 'performanceVerdict': 'Requires memory trend and input review'}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('identity', type=Path)
    args = parser.parse_args()
    report = audit(args.identity)
    identity = read_json(args.identity)
    (Path(identity['root']) / 'data-audit.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps(report))
