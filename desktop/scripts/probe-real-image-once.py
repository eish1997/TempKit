"""One authorized protocol probe. Never retries; never prints credentials/URLs.
Not a substitute for QuickGen's native end-to-end acceptance.
"""
import base64, ctypes, hashlib, json, pathlib, sqlite3, time, urllib.request, urllib.parse, uuid
from ctypes import wintypes

ROOT = pathlib.Path(__file__).resolve().parents[2]
SOURCE = ROOT/'desktop/evidence/generation/g0/native-connected-e211d75a0bde4190b83eafad5c5599e5'
OUT = ROOT/'desktop/evidence/generation/g2/real-probe-01'

class Blob(ctypes.Structure):
    _fields_ = [('cbData', wintypes.DWORD), ('pbData', ctypes.POINTER(ctypes.c_ubyte))]

def crypt(data, entropy, protect=False):
    a=ctypes.create_string_buffer(data); b=ctypes.create_string_buffer(entropy)
    source=Blob(len(data),ctypes.cast(a,ctypes.POINTER(ctypes.c_ubyte)))
    salt=Blob(len(entropy),ctypes.cast(b,ctypes.POINTER(ctypes.c_ubyte))); target=Blob()
    api=ctypes.windll.crypt32.CryptProtectData if protect else ctypes.windll.crypt32.CryptUnprotectData
    if not api(ctypes.byref(source),None,ctypes.byref(salt),None,None,1,ctypes.byref(target)):
        raise RuntimeError('credential envelope unavailable')
    try: return ctypes.string_at(target.pbData,target.cbData)
    finally:
        ctypes.memset(target.pbData,0,target.cbData)
        ctypes.windll.kernel32.LocalFree.argtypes=[ctypes.c_void_p]
        ctypes.windll.kernel32.LocalFree(target.pbData)

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self,*args,**kwargs): return None

def run():
    OUT.mkdir(parents=True,exist_ok=True)
    # Durable admission before obtaining a token or sending bytes; manual review
    # required after any failure, including an unknown HTTP outcome.
    with (OUT/'attempt.json').open('x',encoding='utf-8') as f:
        json.dump({'createdAt':time.time(),'maximumRequests':1,'authorization':'User: directly test; overall CNY2 remains','state':'admitted_no_automatic_retry'},f);f.flush()
        import os
        os.fsync(f.fileno())
    config=json.loads((SOURCE/'config.json').read_text(encoding='utf-8-sig'))
    card=next(a for a in config['abilities'] if a['id']=='clay')
    ref=card['imageBinding'];params=card['imageParameters']
    assert params=={'size':'1024x1024','quality':'low'}
    db=sqlite3.connect((SOURCE/'resource-store/metadata.sqlite').as_uri()+'?mode=ro',uri=True)
    raw,revoked=db.execute('select connection_json,revoked from gateway_connections where connection_id=?',(ref['connection_id'],)).fetchone();assert not revoked
    connection=json.loads(raw)
    profile=json.loads(db.execute('select profile_json from gateway_profiles where profile_id=?',(connection['profile_id'],)).fetchone()[0])
    assert profile['base_url'].rstrip('/')=='http://127.0.0.1:3000'
    binding=json.loads(db.execute('select binding_json from image_bindings where connection_id=? and capability_id=? and revision=?',(ref['connection_id'],ref['capability_id'],ref['revision'])).fetchone()[0]);assert binding['policy']['model']=='gpt-image-2'
    rotated=db.execute('select credential_ref from gateway_credential_rotations where connection_id=? order by revision desc limit 1',(ref['connection_id'],)).fetchone()
    credential=rotated[0] if rotated else connection['credential_ref']
    key=json.dumps([profile['connection_id'],profile['profile_id'],profile['account_id'],credential],ensure_ascii=False,separators=(',',':'))
    vault=SOURCE/'resource-store/credentials'
    entropy=('QuickGen credential v1|'+('\\\\?\\'+str(vault))+'|'+key).encode()
    token=crypt((vault/(hashlib.sha256(key.encode()).hexdigest()+'.dpapi')).read_bytes(),entropy).decode('ascii')
    image=(ROOT/'desktop/evidence/generation/g2/real-inputs/clay-block-a.png').read_bytes()
    boundary='qg-real-probe-'+uuid.uuid4().hex
    fields={'model':'gpt-image-2','prompt':card['prompt'],'n':'1','size':'1024x1024','quality':'low'}
    parts=[]
    for k,v in fields.items():parts.append(f'--{boundary}\r\nContent-Disposition: form-data; name="{k}"\r\n\r\n{v}\r\n'.encode())
    parts.append(f'--{boundary}\r\nContent-Disposition: form-data; name="image"; filename="clay-block-a.png"\r\nContent-Type: image/png\r\n\r\n'.encode()+image+f'\r\n--{boundary}--\r\n'.encode())
    request=urllib.request.Request(profile['base_url'].rstrip('/')+'/v1/images/edits',data=b''.join(parts),headers={'Authorization':'Bearer '+token,'Content-Type':'multipart/form-data; boundary='+boundary},method='POST')
    opener=urllib.request.build_opener(NoRedirect());start=time.time()
    with opener.open(request,timeout=180) as response:
        raw=response.read(28*1024*1024+1);assert len(raw)<=28*1024*1024
    del token,request
    # Signed result descriptors are protected at rest, never copied to logs.
    (OUT/'response.dpapi').write_bytes(crypt(raw,b'QuickGen authorized real probe 01',True))
    value=json.loads(raw);data=value['data'];assert len(data)==1
    item=data[0]
    if item.get('b64_json'):
        result=base64.b64decode(item['b64_json'],validate=True)
    else:
        url=item['url'];parsed=urllib.parse.urlsplit(url)
        assert parsed.scheme=='https' and parsed.hostname=='file.302.ai' and parsed.port in (None,443) and not parsed.username and not parsed.password
        with opener.open(urllib.request.Request(url),timeout=60) as response:result=response.read(20*1024*1024+1)
    assert 0<len(result)<=20*1024*1024
    from PIL import Image
    import io
    im=Image.open(io.BytesIO(result));im.verify()
    (OUT/'result.png').write_bytes(result)
    usage=value.get('usage',{})
    record={'state':'received','requestCount':1,'elapsedSeconds':round(time.time()-start,2),'inputSHA256':hashlib.sha256(image).hexdigest(),'outputSHA256':hashlib.sha256(result).hexdigest(),'outputBytes':len(result),'outputSize':list(im.size),'usage':usage,'nativeAcceptance':False}
    (OUT/'result.json').write_text(json.dumps(record,indent=2),encoding='utf-8');print(json.dumps(record))

if __name__=='__main__':
    try:run()
    except Exception as e:
        # Do not echo request objects, tokens, provider bodies or signed URLs.
        record={'state':'requires_review_no_retry','exceptionType':type(e).__name__}
        if isinstance(e,urllib.error.HTTPError):record['httpStatus']=e.code
        OUT.mkdir(parents=True,exist_ok=True);(OUT/'failure.json').write_text(json.dumps(record),encoding='utf-8');print(json.dumps(record));raise SystemExit(1)
