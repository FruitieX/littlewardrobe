"""Outbound-only Little Wardrobe companion, using the dedicated personal Hermes login."""
import base64
import getpass
import importlib.util
import json
import logging
import os
from pathlib import Path
import re
import sys
import threading
import time
from urllib.parse import urlsplit

logging.disable(logging.CRITICAL)
os.umask(0o077)
spec = importlib.util.spec_from_file_location('littlewardrobe_personal', Path(__file__).with_name('hermes-personal.py'))
personal = importlib.util.module_from_spec(spec)
spec.loader.exec_module(personal)

def emit(phase, **values):
    print(json.dumps({'phase':phase,**values}), flush=True)

def validate_config(config):
    route=urlsplit(config['url'])
    if route.scheme!='https' and not (route.scheme=='http' and route.hostname in ('127.0.0.1','localhost')):
        raise ValueError('Use private HTTPS, or localhost for development')
    if not route.hostname or route.username or route.password or route.query or route.fragment or route.path not in ('','/'):
        raise ValueError('Use the app origin without credentials or paths')
    if not re.fullmatch(r'[0-9a-f]{64}',config['token']):
        raise ValueError('Invalid pairing token')
    config['url']=config['url'].rstrip('/')
    return config

def chat_body(body):
    instructions='\n'.join(message['content'] for message in body['messages'] if message['role'] in ('system','developer')) or 'You are a helpful family shopping assistant.'
    def user_content(content):
        if isinstance(content,str):
            return [{'type':'input_text','text':content}]
        parts=[]
        for part in content:
            if part['type']=='text':
                parts.append({'type':'input_text','text':part['text']})
            elif part['type']=='image_url':
                url=part['image_url']['url']
                if not re.match(r'^data:image/(jpeg|png|webp);base64,',url):
                    raise ValueError('Only inline household images are supported')
                parts.append({'type':'input_image','image_url':url,'detail':part['image_url'].get('detail','high')})
            else:
                raise ValueError('Unsupported user content')
        return parts
    inputs=[{'role':message['role'],'content':user_content(message['content'])} for message in body['messages'] if message['role']=='user']
    if not inputs or not isinstance(body.get('model'),str):
        raise ValueError('Missing model or message')
    result={'model':body['model'],'instructions':instructions,'input':inputs,'store':False,'stream':True}
    effort=body.get('reasoning_effort')
    if effort:
        if effort not in ('none','minimal','low','medium','high','xhigh','max'):
            raise ValueError('Invalid reasoning effort')
        result['reasoning']={'effort':effort}
    response_format=body.get('response_format',{})
    if response_format.get('type')=='json_schema':
        definition=response_format['json_schema']
        result['text']={'format':{'type':'json_schema','name':definition['name'],'strict':definition['strict'],'schema':definition['schema']}}
    elif response_format.get('type')=='json_object':
        result['text']={'format':{'type':'json_object'}}
    return result

def stream_text(lines):
    chunks=[]
    count=0
    for line in lines:
        if not line.startswith('data:'):
            continue
        data=line[5:].strip()
        if not data or data=='[DONE]':
            continue
        event=json.loads(data)
        kind=event.get('type')
        if kind in ('error','response.failed','response.incomplete'):
            raise RuntimeError('Native chat failed')
        if kind=='response.output_text.delta':
            text=event.get('delta','')
            count+=len(text)
            if count>1024*1024:
                raise ValueError('Chat response too large')
            chunks.append(text)
        if kind=='response.completed':
            if event.get('response',{}).get('status') not in ('completed',None):
                raise RuntimeError('Native chat did not complete')
            text=''.join(part.get('text','') for item in event.get('response',{}).get('output',[]) if item.get('type')=='message' for part in item.get('content',[]) if part.get('type')=='output_text') or ''.join(chunks)
            if not text or len(text)>1024*1024:
                raise ValueError('Missing or oversized chat text')
            return {'choices':[{'message':{'role':'assistant','content':text}}]}
    raise RuntimeError('Chat stream interrupted')

def perform(job):
    import httpx
    from agent.codex_headers import codex_cloudflare_headers
    path=job['path']
    body=job.get('body') or {}
    token,base=personal.personal_credential(refresh=True)
    if path in ('images/generations','images/edits'):
        model=body.get('model')
        if model not in ('gpt-image-2-low','gpt-image-2-medium','gpt-image-2-high'):
            raise ValueError('Unsupported Hermes image model')
        aspect={'1536x1024':'landscape','1024x1536':'portrait'}.get(body.get('size'),'square')
        references=body.get('reference_images')
        if path=='images/edits':
            images=body.get('image',[])
            references=[entry['image_url'] for entry in images if isinstance(entry,dict) and isinstance(entry.get('image_url'),str) and entry['image_url'].startswith('data:image/')]
            if len(references)!=len(images) or not references:
                raise ValueError('Image editing needs inline reference images')
        result=personal.image_provider(refresh=True,model=model,background=body.get('background'),size=body.get('size')).generate(body['prompt'],aspect_ratio=aspect,reference_image_urls=references)
        if not result.get('success'):
            raise RuntimeError('Image provider failed')
        picture=Path(result['image'])
        if picture.stat().st_size>20*1024*1024:
            raise ValueError('Image too large')
        return {'data':[{'b64_json':base64.b64encode(picture.read_bytes()).decode('ascii')}]}
    headers={**codex_cloudflare_headers(token,base_url=base),'Authorization':f'Bearer {token}'}
    with httpx.Client(headers=headers,timeout=httpx.Timeout(300,connect=20),follow_redirects=False) as client:
        if path=='models':
            response=client.get(f'{base}/models',params={'client_version':'0.159.3'})
            response.raise_for_status()
            if len(response.content)>2*1024*1024:
                raise ValueError('Model catalog too large')
            models=response.json().get('models',[])
            return {'data':[{'id':model['slug']} for model in models[:1000] if isinstance(model.get('slug'),str)]}
        if path!='chat/completions':
            raise ValueError('Unknown job type')
        with client.stream('POST',f'{base}/responses',json=chat_body(body)) as response:
            response.raise_for_status()
            return stream_text(response.iter_lines())

def main():
    from hermes_constants import get_hermes_home
    personal.personal_credential(refresh=True)
    home=get_hermes_home()
    file=home/'littlewardrobe-companion.json'
    if '--pair-stdin' in sys.argv:
        config=validate_config(json.load(sys.stdin))
    elif '--setup' in sys.argv:
        config=validate_config({'url':sys.argv[sys.argv.index('--setup')+1],'token':getpass.getpass('Little Wardrobe pairing token: ')})
    else:
        stat=file.stat()
        if stat.st_uid!=os.getuid() or stat.st_mode&0o077:
            raise RuntimeError('Companion pairing file must be private and owned by this user')
        config=validate_config(json.loads(file.read_text()))
    if '--setup' in sys.argv or '--pair-stdin' in sys.argv:
        if file.is_symlink():
            raise RuntimeError('Refusing a symlink pairing file')
        temporary=file.with_suffix('.tmp')
        descriptor=os.open(temporary,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
        with os.fdopen(descriptor,'w') as output:
            json.dump(config,output)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary,file)
        emit('paired',profile=home.name)
        return
    import httpx
    headers={'Authorization':f"Bearer {config['token']}",'X-LittleWardrobe-Request':'1','Origin':config['url']}
    stop=threading.Event()
    def call(client,path,body):
        response=client.post(f"{config['url']}/api/companion/{path}",json=body)
        response.raise_for_status()
        if len(response.content)>9*1024*1024:
            raise ValueError('Companion response too large')
        return response.json()
    def heartbeat():
        with httpx.Client(headers=headers,timeout=20,follow_redirects=False) as client:
            while not stop.is_set():
                try:call(client,'heartbeat',{})
                except Exception:pass
                stop.wait(10)
    threading.Thread(target=heartbeat,daemon=True).start()
    emit('started',profile=home.name)
    try:
        with httpx.Client(headers=headers,timeout=30,follow_redirects=False) as client:
            while True:
                try:
                    job=call(client,'claim',{}).get('job')
                except Exception:
                    emit('connection-unavailable')
                    time.sleep(10)
                    continue
                if not job:
                    time.sleep(3)
                    continue
                started=time.monotonic()
                completion={'id':job['id'],'lease':job['lease'],'result':None,'failed':True}
                try:
                    completion['result']=perform(job)
                    completion['failed']=False
                except (Exception,SystemExit) as error:
                    # Provider errors may contain private account identifiers and bodies.
                    metadata={'httpStatus':error.response.status_code} if isinstance(error,httpx.HTTPStatusError) else {}
                    emit('job-failed',kind=job['path'],errorType=type(error).__name__,**metadata)
                emit('job-finished',kind=job['path'],success=not completion['failed'],seconds=round(time.monotonic()-started,1))
                # Retry delivery, never generation, after a transient app/network failure.
                for _ in range(6):
                    try:
                        call(client,'complete',completion)
                        break
                    except httpx.HTTPStatusError as error:
                        if error.response.status_code in (401,403,409):
                            break
                    except Exception:pass
                    time.sleep(3)
    finally:stop.set()

if __name__=='__main__':
    try:main()
    except (Exception,SystemExit) as error:
        emit('stopped',errorType=type(error).__name__)
        sys.exit(1)
