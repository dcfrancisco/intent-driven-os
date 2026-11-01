#!/usr/bin/env python3
"""Bounded live CPU/API recovery acceptance. Cancels only its own requests.

Requires an idle daemon. Credentials are read from a private file, never logged.
Run against a disposable instance to qualify overload without disturbing clients.
"""
import concurrent.futures
import http.client
import json
import os
import pathlib
import socket
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid

BASE = os.environ.get('TALVIK_TEST_BASE_URL', 'http://127.0.0.1:11435').rstrip('/')
TOKEN = pathlib.Path(os.environ.get('TALVIK_TEST_TOKEN_FILE', str(pathlib.Path.home()/'.marina/sse-test.token'))).read_text().strip()
MODEL = os.environ.get('TALVIK_TEST_CODER_MODEL', 'qwen2.5-coder-3b-instruct-q4_k_m')
PREFIX = 'rc-recovery-' + uuid.uuid4().hex[:10]
RESULTS = []
OWNED = set()


def request(path, body=None, auth=True):
    headers = {'Content-Type': 'application/json'}
    if auth:
        headers['Authorization'] = 'Bearer ' + TOKEN
    req = urllib.request.Request(BASE+path, data=json.dumps(body).encode() if body is not None else None, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=120) as response:
            return response.status, response.read().decode()
    except urllib.error.HTTPError as error:
        return error.code, error.read().decode()


def state():
    status, wire = request('/api/ps')
    assert status == 200, (status, wire)
    body = json.loads(wire)
    return {key: body[key] for key in ('active_requests', 'pending_requests', 'queue_capacity')}


def idle(seconds=15):
    end = time.monotonic()+seconds
    while time.monotonic() < end:
        current = state()
        if not current['active_requests'] and not current['pending_requests']:
            return current
        time.sleep(.1)
    raise AssertionError('scheduler did not return to idle: '+str(current))


def body(**kwargs):
    return {'model':MODEL, 'messages':[{'role':'user','content':'Say hello briefly.'}],
            'max_tokens':8, 'context_size':512, 'temperature':0, **kwargs}


def completion():
    status, wire = request('/v1/chat/completions', body())
    assert status == 200, (status, wire)
    data = json.loads(wire)
    assert data['choices'][0]['message']['content'], data
    return {'status':status, 'finish_reason':data['choices'][0]['finish_reason']}


def check(name, run):
    started=time.monotonic()
    try:
        details=run()
        result={'test':name,'passed':True,'details':details}
    except Exception as error:
        result={'test':name,'passed':False,'error':str(error)}
    result['seconds']=round(time.monotonic()-started,3)
    RESULTS.append(result)
    print(json.dumps(result),flush=True)


def status_check(expected, path='/v1/chat/completions', payload=None, auth=True):
    status, wire=request(path,payload,auth)
    assert status == expected, (status,wire)
    data=json.loads(wire)
    assert isinstance(data.get('error'),dict), data
    assert data['error'].get('message'), data
    return {'status':status}


def stream_frames(wire):
    frames=[]
    for line in wire.splitlines():
        if line.startswith('data: ') and line != 'data: [DONE]':
            frames.append(json.loads(line[6:]))
    return frames


def timeout_recovery():
    status,wire=request('/v1/chat/completions',body(timeout_ms=1,max_tokens=512))
    assert status==504,(status,wire)
    idle()
    return {'timeout_status':status,'next_request':completion()}


def stream_timeout():
    status,wire=request('/v1/chat/completions',body(stream=True,timeout_ms=1,max_tokens=512))
    assert status==200,(status,wire)
    errors=[f['error'] for f in stream_frames(wire) if 'error' in f]
    assert errors and errors[0].get('code')=='generation_timeout', wire
    idle()
    return {'error_code':errors[0]['code'],'next_request':completion()}


def output_limit():
    status,wire=request('/v1/chat/completions',body(max_tokens=1,stream=True,stream_options={'include_usage':True}))
    assert status==200 and 'data: [DONE]' in wire,(status,wire)
    frames=stream_frames(wire)
    finishes=[c.get('finish_reason') for f in frames for c in f.get('choices',[]) if c.get('finish_reason')]
    assert finishes==['length'],finishes
    assert any(f.get('usage') for f in frames),wire
    return {'finish_reason':'length','usage':True}


def overflow():
    status,wire=request('/v1/chat/completions',body(context_size=512,messages=[{'role':'user','content':'long context token '*600}]))
    assert status==400,(status,wire)
    assert 'context' in json.loads(wire)['error']['message'], wire
    idle()
    return {'status':status,'next_request':completion()}


def open_stream(request_id, route='/v1/chat/completions'):
    OWNED.add(request_id)
    if route.startswith('/v1/'):
        payload=body(request_id=request_id,stream=True,max_tokens=1024,context_size=4096,
                     messages=[{'role':'user','content':'List the integers from 1 through 10000, one at a time.'}])
    else:
        payload={'model':MODEL,'request_id':request_id,'prompt':'List integers from 1 through 10000.',
                 'stream':True,'options':{'num_predict':1024,'num_ctx':4096}}
    url=urllib.parse.urlparse(BASE)
    assert url.scheme=='http','Raw disconnect tests require plain HTTP'
    conn=http.client.HTTPConnection(url.hostname,url.port,timeout=60)
    conn.request('POST',route,json.dumps(payload),{'Authorization':'Bearer '+TOKEN,'Content-Type':'application/json'})
    response=conn.getresponse()
    assert response.status==200,(response.status,response.read().decode())
    return conn,response


def disconnect(route,after_token=False):
    request_id=PREFIX+'-disconnect-'+str(len(RESULTS))
    conn,response=open_stream(request_id,route)
    if after_token:
        while True:
            line=response.readline().decode()
            assert line,'Stream ended without content'
            if line.startswith('data: ') and 'content' in line:
                break
    response.close();conn.close()
    idle()
    OWNED.discard(request_id)
    return {'next_request':completion()}


def queue_overflow():
    capacity=state()['queue_capacity']
    assert 0 < capacity <= 16, 'Refusing unbounded load test'
    request_id=PREFIX+'-active'
    conn,response=open_stream(request_id)
    ids=[PREFIX+'-queued-'+str(i) for i in range(capacity)]
    OWNED.update(ids)
    def queued(identifier):
        return request('/v1/chat/completions',body(request_id=identifier,timeout_ms=15000))
    try:
        with concurrent.futures.ThreadPoolExecutor(max_workers=capacity) as pool:
            futures=[pool.submit(queued,identifier) for identifier in ids]
            end=time.monotonic()+10
            while state()['pending_requests'] < capacity and time.monotonic()<end:
                time.sleep(.05)
            assert state()['pending_requests']==capacity,state()
            duplicate_status,duplicate_wire=request('/v1/chat/completions',body(request_id=request_id))
            assert duplicate_status==409,(duplicate_status,duplicate_wire)
            status,wire=request('/v1/chat/completions',body())
            assert status==429,(status,wire)
            for identifier in ids+[request_id]:
                code,_=request('/v1/generations/'+identifier+'/cancel',{})
                assert code in (202,404),(identifier,code)
            response.close();conn.close()
            for future in futures:
                code,_=future.result(timeout=25)
                assert code in (200,504),code
            idle()
            return {'overflow_status':status,'capacity':capacity,'next_request':completion()}
    finally:
        response.close();conn.close()
        for identifier in ids+[request_id]:
            request('/v1/generations/'+identifier+'/cancel',{})
            OWNED.discard(identifier)


def main():
    current=state()
    if current['active_requests'] or current['pending_requests']:
        raise SystemExit('Daemon is busy; no tests run. Retry when idle.')
    checks=[
        ('auth_required',lambda:status_check(401,'/v1/models',auth=False)),
        ('missing_model',lambda:status_check(400,payload={'messages':[{'role':'user','content':'hi'}]})),
        ('unknown_model',lambda:status_check(404,payload=body(model='rc-nonexistent-model'))),
        ('unsupported_option',lambda:status_check(501,payload=body(logprobs=True))),
        ('invalid_output_limit',lambda:status_check(400,payload=body(max_tokens=0))),
        ('invalid_context_limit',lambda:status_check(400,payload=body(context_size=999999))),
        ('output_limit_sse',output_limit),
        ('prompt_overflow_recovery',overflow),
        ('deadline_recovery',timeout_recovery),
        ('stream_deadline_recovery',stream_timeout),
        ('disconnect_before_token',lambda:disconnect('/v1/chat/completions')),
        ('disconnect_after_token',lambda:disconnect('/v1/chat/completions',True)),
        ('ollama_disconnect_before_token',lambda:disconnect('/api/generate')),
        ('queue_overflow_and_cancel_recovery',queue_overflow),
    ]
    try:
        for name,run in checks:
            check(name,run)
            idle()
    finally:
        for identifier in OWNED:
            request('/v1/generations/'+identifier+'/cancel',{})
        path=pathlib.Path(os.environ.get('TALVIK_TEST_REPORT','talvik-api-recovery-report.json'))
        path.write_text(json.dumps({'base_url':BASE,'model':MODEL,'results':RESULTS},indent=2)+'\n')
    raise SystemExit(0 if len(RESULTS)==len(checks) and all(r['passed'] for r in RESULTS) else 1)

if __name__=='__main__':
    main()
