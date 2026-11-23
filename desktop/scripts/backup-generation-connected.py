"""Verified backup of this stage's named isolated directory; never production."""
import ctypes, hashlib, json, pathlib, shutil, sqlite3, stat, uuid
from ctypes import wintypes

BASE=pathlib.Path(__file__).resolve().parents[2]
SOURCE=BASE/'desktop/evidence/generation/g0/native-connected-e211d75a0bde4190b83eafad5c5599e5'
DEST=BASE/'desktop/evidence/generation/g2'

def digest(p):
    h=hashlib.sha256()
    with p.open('rb') as f:
        for chunk in iter(lambda:f.read(1024*1024),b''):h.update(chunk)
    return h.hexdigest()

def inventory():
    for p in [SOURCE,*SOURCE.rglob('*')]:
        if p.lstat().st_file_attributes & stat.FILE_ATTRIBUTE_REPARSE_POINT:
            raise RuntimeError('redirected source rejected')
    return sorted(p.relative_to(SOURCE).as_posix() for p in SOURCE.rglob('*') if p.is_file())

def database(root):
    with sqlite3.connect((root/'resource-store/metadata.sqlite').as_uri()+'?mode=ro',uri=True) as db:
        integrity=db.execute('pragma integrity_check').fetchone()[0]
        foreign=db.execute('pragma foreign_key_check').fetchall()
        if integrity!='ok' or foreign:raise RuntimeError('database validation failed')
        counts={t:db.execute('select count(*) from '+t).fetchone()[0] for t in ['tasks','resources','gateway_connections','image_budget_reservations']}
        return {'schema':db.execute('pragma user_version').fetchone()[0],'integrity':integrity,'foreignKeyErrors':len(foreign),'counts':counts}

def main():
    if (SOURCE/'quickgen-isolated-v1').read_bytes()!=b'QuickGen isolated test data v1':raise RuntimeError('isolated marker mismatch')
    inventory()
    api=ctypes.WinDLL('kernel32',use_last_error=True)
    api.CreateFileW.argtypes=[wintypes.LPCWSTR,wintypes.DWORD,wintypes.DWORD,ctypes.c_void_p,wintypes.DWORD,wintypes.DWORD,ctypes.c_void_p]
    api.CreateFileW.restype=wintypes.HANDLE
    api.CloseHandle.argtypes=[wintypes.HANDLE]
    # Read sharing permits copying the lock itself, but blocks any host writer.
    handle=api.CreateFileW(str(SOURCE/'resource-store/writer.lock'),0x80000000,1,None,3,0x00200000,None)
    if handle==ctypes.c_void_p(-1).value:raise RuntimeError('host writer is still open; normal exit required')
    try:
        before=database(SOURCE);files=inventory()
        target=DEST/('connected-before-A913-'+uuid.uuid4().hex);target.mkdir()
        manifest=[]
        for relative in files:
            src=SOURCE/relative;dst=target/relative;expected=digest(src)
            dst.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(src,dst)
            if digest(dst)!=expected or digest(src)!=expected:raise RuntimeError('source changed or copy mismatch')
            manifest.append({'relative':relative,'bytes':src.stat().st_size,'sha256':expected})
        after=database(target)
        if before!=after or inventory()!=files:raise RuntimeError('backup inventory/database mismatch')
        for record in manifest:
            if digest(SOURCE/record['relative'])!=record['sha256'] or digest(target/record['relative'])!=record['sha256']:
                raise RuntimeError('final backup validation failed')
        (target/'backup-manifest.json').write_text(json.dumps(manifest,indent=2),encoding='utf-8')
        report={'source':str(SOURCE),'backup':str(target),'fileCount':len(manifest),'bytes':sum(r['bytes'] for r in manifest),'database':before,'manifestSHA256':digest(target/'backup-manifest.json'),'allSourceAndCopyHashesMatch':True}
        (DEST/'connected-before-A913-verified.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
        print(json.dumps(report))
    finally:api.CloseHandle(handle)

if __name__=='__main__':
    try:main()
    except Exception as e:
        print(json.dumps({'error':str(e),'verified':False}));raise SystemExit(1)
