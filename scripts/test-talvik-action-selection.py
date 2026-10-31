#!/usr/bin/env python3
"""Live tool-selection regression; never executes model-generated commands."""
import json
import os
from pathlib import Path
import shlex
import time
import urllib.request

BASE = os.getenv('TALVIK_TEST_BASE', 'http://127.0.0.1:11435').rstrip('/')
TOKEN = Path(os.getenv('TALVIK_TEST_TOKEN_FILE', str(Path.home() / '.marina/sse-test.token'))).read_text().strip()
MODEL = os.getenv('TALVIK_TEST_MODEL', 'qwen3-1.7b-q4_k_m')
PRESET = Path(__file__).resolve().parents[1] / 'docs/examples/vscode/talvik-coding.agent.md'
SYSTEM = PRESET.read_text().split('---', 2)[2].strip() + '\nWorkspace context: this is a Maven project with pom.xml. Maven is available. No Maven wrapper is present.'
TOOL = {'type': 'function', 'function': {'name': 'run_in_terminal', 'description': 'Execute a shell command in the workspace terminal.', 'parameters': {'type': 'object', 'properties': {'command': {'type': 'string'}, 'explanation': {'type': 'string'}, 'isBackground': {'type': 'boolean'}}, 'required': ['command', 'explanation', 'isBackground']}}}

def request(path, body=None):
    req = urllib.request.Request(BASE + path, data=json.dumps(body).encode() if body is not None else None, headers={'Authorization': 'Bearer ' + TOKEN, 'Content-Type': 'application/json'})
    with urllib.request.urlopen(req, timeout=180) as response:
        return json.load(response)

def build_call(message):
    calls = message.get('tool_calls') or []
    if len(calls) != 1 or calls[0]['function']['name'] != 'run_in_terminal':
        return False
    args = json.loads(calls[0]['function']['arguments'])
    command = shlex.split(args.get('command', ''))
    return command in (['mvn', '-DskipTests', 'package'], ['mvn', '-DskipTests=true', 'package']) and args.get('isBackground') is False

ps = request('/api/ps')
assert ps['active_requests'] == ps['pending_requests'] == 0, 'Daemon busy: do not compete with live requests'
user = {'role': 'user', 'content': "build this project, don't check anything. just build"}
call = {'id': 'call_build', 'type': 'function', 'function': {'name': 'run_in_terminal', 'arguments': json.dumps({'command': 'mvn -DskipTests package', 'explanation': 'Build only, skipping tests.', 'isBackground': False})}}
cases = [
    ('build_only', [user], build_call),
    ('retry_after_narration', [user, {'role': 'assistant', 'content': 'Building the project...'}, {'role': 'user', 'content': 'No build ran. Execute the build now, without tests or inspection.'}], build_call),
    ('ordinary_question', [{'role': 'user', 'content': 'What is 2+2? Reply with the number only.'}], lambda m: not m.get('tool_calls') and m.get('content', '').strip() == '4'),
    ('completed_build', [user, {'role': 'assistant', 'content': None, 'tool_calls': [call]}, {'role': 'tool', 'tool_call_id': 'call_build', 'name': 'run_in_terminal', 'content': 'Exit code: 0\nBUILD SUCCESS\nTests were skipped.'}], lambda m: not m.get('tool_calls') and 'success' in m.get('content', '').lower()),
]
results = []
for name, messages, check in cases:
    start = time.monotonic()
    response = request('/v1/chat/completions', {'model': MODEL, 'messages': [{'role': 'system', 'content': SYSTEM}] + messages, 'tools': [TOOL], 'tool_choice': 'auto', 'temperature': 0, 'max_tokens': 192, 'context_size': 4096, 'chat_template_kwargs': {'enable_thinking': False}})
    message = response['choices'][0]['message']
    result = {'test': name, 'passed': check(message), 'seconds': round(time.monotonic() - start, 3), 'message': message}
    results.append(result)
    print(json.dumps(result), flush=True)
report = {'model': MODEL, 'execution': 'selection-only; no commands executed; supplied build result is a synthetic fixture', 'passed': all(r['passed'] for r in results), 'results': results}
if path := os.getenv('TALVIK_TEST_REPORT'):
    Path(path).write_text(json.dumps(report, indent=2) + '\n')
raise SystemExit(0 if report['passed'] else 1)
