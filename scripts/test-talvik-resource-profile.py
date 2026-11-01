#!/usr/bin/env python3
"""Native controlled-budget and manual-load readiness acceptance.
Requires the canonical installed daemon idle with DeepSeek R1 1.5B resident.
Creates a disposable loopback daemon/state directory, never exhausts host RAM,
and always terminates that instance. Set TALVIK_TEST_OUTPUT_DIR for artifacts.
"""
import json,pathlib,urllib.request,urllib.error,time,tempfile,subprocess,os,socket
HOME=pathlib.Path.home();OUT=pathlib.Path(os.environ.get('TALVIK_TEST_OUTPUT_DIR', '.')).resolve();OUT.mkdir(parents=True,exist_ok=True)
token=(HOME/'.marina/sse-test.token').read_text().strip()
def call(base,path,body=None,key=None):
 headers={'Content-Type':'application/json'}
 if key:headers['Authorization']='Bearer '+key
 r=urllib.request.Request(base+path,data=json.dumps(body).encode() if body is not None else None,headers=headers)
 try:
  with urllib.request.urlopen(r,timeout=90) as f:return f.status,json.load(f)
 except urllib.error.HTTPError as e:return e.code,json.load(e)
base='http://127.0.0.1:11435'
for _ in range(100):
 _,ps=call(base,'/api/ps',key=token)
 if not ps['active_requests'] and not ps['pending_requests']:break
 time.sleep(.1)
else:raise SystemExit('Real daemon did not return to idle')
status,result=call(base,'/v1/chat/completions',{'model':'qwen2.5-coder-3b-instruct-q4_k_m','messages':[{'role':'user','content':'Say hello briefly.'}],'max_tokens':8,'context_size':512},token)
assert status==200 and result['choices'][0]['message']['content'],(status,result)
print('Original service recovered after IDE cancellation',flush=True)
_,ps=call(base,'/api/ps',key=token)
model=next(m for m in ps['models'] if m['name']=='deepseek-r1-1.5b');size=model['size'];budget=2*size+256*1048576
with tempfile.TemporaryDirectory(prefix='talvik-budget-',dir='/tmp') as directory:
 root=pathlib.Path(directory);(root/'models').mkdir();(root/'models/deepseek-r1-1.5b.gguf').symlink_to(HOME/'.marina/models/deepseek-r1-1.5b.gguf')
 (root/'config.yaml').write_text('default_model: none\nauto_download: false\nauto_load: false\n')
 with socket.socket() as listener:listener.bind(('127.0.0.1',0));port=listener.getsockname()[1]
 target='http://127.0.0.1:'+str(port)
 env={k:v for k,v in os.environ.items() if not k.startswith(('TALVIK_','MARINA_','OID_')) and k not in ('DYLD_LIBRARY_PATH','LD_LIBRARY_PATH','LLAMA_CPP_LIB_DIR')}
 env.update(TALVIK_HOME=str(root),TALVIK_STATE_DIR=str(root),TALVIK_CONFIG=str(root/'config.yaml'),TALVIK_MODEL_DIR=str(root/'models'),TALVIK_REGISTRY=str(root/'models.registry'),TALVIK_SOCKET=str(root/'test.sock'),TALVIK_HTTP_ADDR='127.0.0.1:'+str(port),TALVIK_HTTP_AUTH='false',TALVIK_AUTO_DOWNLOAD='false',TALVIK_AUTO_LOAD='false',TALVIK_BACKEND='cpu',TALVIK_PLACEMENT='cpu_only',TALVIK_MEMORY_BUDGET_BYTES=str(budget),TALVIK_SAFETY_MARGIN_BYTES='0',TALVIK_AUTOMATIC_EVICTION='false',TALVIK_THREADS='8')
 log=open(OUT/'talvik-resource-profile-daemon.log','w')
 process=subprocess.Popen([str(HOME/'.marina/bin/talvikd')],env=env,stdout=log,stderr=subprocess.STDOUT)
 try:
  for _ in range(100):
   try:
    code,_=call(target,'/healthz')
    if code==200:break
   except Exception:pass
   assert process.poll() is None,'Test daemon exited'
   time.sleep(.1)
  else:raise AssertionError('test daemon readiness timeout')
  code,_=call(target,'/readyz');assert code==503,code
  code,loaded=call(target,'/v1/models/deepseek-r1-1.5b/load',{});assert code==200,(code,loaded)
  code,_=call(target,'/readyz');assert code==200,code
  subprocess.run([str(HOME/'.marina/bin/talvik'),'--json','unload','deepseek-r1-1.5b'],env=env,stdout=subprocess.DEVNULL,check=True,timeout=15)
  code,_=call(target,'/readyz');assert code==503,code
  code,loaded=call(target,'/v1/models/deepseek-r1-1.5b/load',{});assert code==200,(code,loaded)
  code,_=call(target,'/readyz');assert code==200,code
  payload={'model':'deepseek-r1-1.5b','messages':[{'role':'user','content':'Reply hello.'}],'max_tokens':4,'context_size':512,'temperature':0}
  code,wire=call(target,'/v1/chat/completions',payload)
  assert code==200,(code,wire)
  code,rejected=call(target,'/v1/chat/completions',{**payload,'context_size':32768})
  assert code==429 and 'memory budget rejected' in rejected['error']['message'],(code,rejected)
  repeated=[]
  for index in range(12):
   started=time.monotonic();status,wire=call(target,'/v1/chat/completions',{**payload,'messages':[{'role':'user','content':f'Check {index}: reply hello.'}]})
   assert status==200 and wire['choices'][0]['message']['content'],(status,wire)
   repeated.append(round(time.monotonic()-started,3))
  _,state=call(target,'/api/ps');assert state['active_requests']==state['pending_requests']==0,state
  _,identity=call(target,'/v1/version')
  result={'passed':True,'manual_readiness_transitions':[503,200,503,200],'build':identity,'model':'deepseek-r1-1.5b','memory_budget_bytes':budget,'resident_model_bytes':size,'large_context_rejection_status':code,'subsequent_completions':12,'completion_seconds':repeated,'final_active_requests':0,'final_pending_requests':0,'scope':'controlled admission budget; no OS memory exhaustion; serial continuous client'}
  (OUT/'talvik-resource-profile-report.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
 finally:
  process.terminate()
  try:process.wait(timeout=10)
  except subprocess.TimeoutExpired:process.kill();process.wait()
  log.close()
