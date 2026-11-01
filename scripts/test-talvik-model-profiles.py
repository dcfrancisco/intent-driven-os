#!/usr/bin/env python3
"""Serial loopback profile, parser, reasoning and discovery acceptance.

Never executes emitted tools. Cancels only its uniquely identified probe.
"""
import concurrent.futures
import json
import os
import pathlib
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid

BASE = os.environ.get('TALVIK_TEST_BASE_URL', 'http://127.0.0.1:11435').rstrip('/')
TOKEN = pathlib.Path(os.environ.get('TALVIK_TEST_TOKEN_FILE', str(pathlib.Path.home()/'.marina/sse-test.token'))).read_text().strip()
REPORT = pathlib.Path(os.environ.get('TALVIK_TEST_REPORT', 'talvik-model-profiles-report.json'))
CODER = os.environ.get('TALVIK_TEST_CODER_MODEL', 'qwen2.5-coder-3b-instruct-q4_k_m')
VISION = os.environ.get('TALVIK_TEST_VISION_MODEL', 'qwen3.5-4b-Q4_K_M')
REASONER = os.environ.get('TALVIK_TEST_REASONING_MODEL', 'deepseek-r1-1.5b')
PREFIX = 'profiles-'+uuid.uuid4().hex[:10]
RESULTS = []


def request(path, body=None):
    req = urllib.request.Request(BASE+path, data=json.dumps(body).encode() if body is not None else None,
                                 headers={'Authorization':'Bearer '+TOKEN, 'Content-Type':'application/json'})
    try:
        with urllib.request.urlopen(req, timeout=150) as response:
            return response.status, response.read().decode()
    except urllib.error.HTTPError as error:
        return error.code, error.read().decode()


def obj(path, body=None):
    status, wire = request(path, body)
    assert status == 200, (status, wire)
    return json.loads(wire)


def idle():
    state = obj('/api/ps')
    return state['active_requests'] == state['pending_requests'] == 0


def check(name, run):
    started = time.monotonic()
    try:
        details = run()
        result = {'test':name, 'passed':True, 'details':details}
    except Exception as error:
        result = {'test':name, 'passed':False, 'error':str(error)}
    result['seconds'] = round(time.monotonic()-started, 3)
    RESULTS.append(result)
    REPORT.write_text(json.dumps(RESULTS, indent=2)+'\n')
    print(json.dumps(result), flush=True)


def body(model=VISION, **extra):
    return {'model':model, 'messages':[{'role':'user','content':'What is 2 + 2? Reply with the number only.'}],
            'max_tokens':16, 'temperature':0, 'context_size':4096, 'timeout_ms':120000, **extra}


def profile(model):
    rows = obj('/v1/models')['data']
    return next(row['runtime_profile'] for row in rows if row['id'] == model)


def load_profiles():
    for model in [CODER, VISION, REASONER]:
        obj('/v1/chat/completions', body(model))
    coder, vision, reasoner = (profile(m) for m in [CODER, VISION, REASONER])
    assert coder['tool_parser'] == 'xml_json', coder
    assert vision['tool_parser'] == 'xml_function', vision
    assert vision['cache_strategy'] == 'recurrent_checkpoint', vision
    assert coder['cache_strategy'] == reasoner['cache_strategy'] == 'attention'
    assert 'vision' in vision['capabilities'] and 'vision' not in coder['capabilities']
    assert reasoner['reasoning_parser'] == 'think_tags', reasoner
    assert reasoner['tokenizer_adds_bos'], reasoner
    assert not reasoner['thinking_control'], reasoner
    assert 'embedding' not in coder['capabilities']
    return {CODER:coder, VISION:vision, REASONER:reasoner}


def reasoning(streaming, model=VISION):
    extra = {'stream':streaming, 'max_tokens':96}
    if model == VISION:
        extra['chat_template_kwargs'] = {'enable_thinking':True}
    status, wire = request('/v1/chat/completions', body(model, **extra))
    assert status == 200, (status, wire)
    content, thought = '', ''
    if streaming:
        assert 'data: [DONE]' in wire
        for line in wire.splitlines():
            if not line.startswith('data: ') or line == 'data: [DONE]':
                continue
            event = json.loads(line[6:]); assert 'error' not in event, event
            for choice in event.get('choices', []):
                delta = choice.get('delta', {})
                content += delta.get('content') or ''
                thought += delta.get('reasoning_content') or ''
    else:
        message = json.loads(wire)['choices'][0]['message']
        content, thought = message.get('content') or '', message.get('reasoning_content') or ''
    assert thought.strip(), (content, thought)
    assert '<think>' not in content and '</think>' not in content
    assert '<think>' not in thought and '</think>' not in thought
    return {'answer':content, 'reasoning_characters':len(thought)}


def coder_tools(streaming=False):
    tools = [{'type':'function', 'function':{'name':'read_file','description':'Read the requested path.',
              'parameters':{'type':'object','properties':{'path':{'type':'string'}},'required':['path']}}}]
    payload = body(CODER, max_tokens=96, stream=streaming,
                   messages=[{'role':'user','content':'Call read_file with path /tmp/profiles-probe.txt.'}],
                   tools=tools, tool_choice='required')
    status, wire = request('/v1/chat/completions', payload)
    assert status == 200, (status, wire)
    if streaming:
        assert 'data: [DONE]' in wire
        calls = []
        for line in wire.splitlines():
            if line.startswith('data: ') and line != 'data: [DONE]':
                event = json.loads(line[6:]); assert 'error' not in event, event
                for choice in event.get('choices', []):
                    calls.extend(choice.get('delta', {}).get('tool_calls') or [])
        assert len(calls) == 1, calls
        call, finish = calls[0], 'tool_calls'
    else:
        data = json.loads(wire)
        call = data['choices'][0]['message']['tool_calls'][0]
        finish = data['choices'][0]['finish_reason']
    assert call['function']['name'] == 'read_file', call
    assert json.loads(call['function']['arguments'])['path'] == '/tmp/profiles-probe.txt', call
    return {'tool':call['function']['name'], 'finish_reason':finish}


def ollama_thinking():
    data = obj('/api/chat', {'model':VISION,'messages':[{'role':'user','content':'What is 2 + 2? Reply number only.'}],
               'think':False,'stream':False,'options':{'num_ctx':4096,'num_predict':16,'temperature':0}})
    assert '4' in data['message']['content'], data
    status, _ = request('/api/chat', {'model':REASONER, 'messages':[{'role':'user','content':'hello'}], 'think':False,'stream':False})
    assert status == 501, status
    status, _ = request('/v1/chat/completions', body(REASONER, chat_template_kwargs={'enable_thinking':False}))
    assert status == 501, status
    return {'thinking_off_answer':data['message']['content'], 'unsupported_control_status':status}


def discovery_during_generation():
    assert idle(), 'Daemon busy'
    own = PREFIX+'-discovery'
    with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
        future = pool.submit(request, '/v1/chat/completions', body(CODER, request_id=own, max_tokens=128,
                              messages=[{'role':'user','content':'Count from 1 to 1000, separated by commas.'}]))
        try:
            for _ in range(100):
                if not idle():
                    break
                time.sleep(.05)
            else:
                raise AssertionError('Probe never became active')
            # Let the worker acquire its model lock, then inspect all profiles.
            time.sleep(.2)
            started = time.monotonic()
            rows = obj('/v1/models')['data']
            elapsed = time.monotonic()-started
            assert elapsed < 2, elapsed
            assert all(row['runtime_profile'] for row in rows if row.get('resident')), rows
            return {'discovery_seconds':round(elapsed, 3)}
        finally:
            cancel_status, _ = request('/v1/generations/'+own+'/cancel', {})
            assert cancel_status in {200, 202}, cancel_status
            future.result(timeout=15)


assert urllib.parse.urlparse(BASE).hostname in {'127.0.0.1','localhost','::1'}
assert idle(), 'Daemon busy; no tests started'
check('loaded_profiles', load_profiles)
check('coder_json_tool', coder_tools)
check('coder_json_tool_sse', lambda:coder_tools(True))
check('reasoning_nonstream', lambda:reasoning(False))
check('reasoning_sse', lambda:reasoning(True))
check('reasoner_metadata_and_transport', lambda:reasoning(True, REASONER))
check('ollama_thinking_and_unsupported_controls', ollama_thinking)
check('nonblocking_discovery_during_generation', discovery_during_generation)
raise SystemExit(0 if all(r['passed'] for r in RESULTS) else 1)
