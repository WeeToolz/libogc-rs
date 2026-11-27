"""Read the completed arithmetic diagnostic through Dolphin's local GDB stub."""
import hashlib, json, os, pathlib, re, signal, socket, struct, subprocess, sys, time
elf, dol, output = map(lambda p: pathlib.Path(p).resolve(), sys.argv[1:4])
mode = sys.argv[4]
assert mode in ('before', 'candidate')
output.mkdir(parents=True, exist_ok=True)
syms = subprocess.check_output([os.environ.get('READELF','readelf'), '-Ws', str(elf)], text=True)
result = re.search(r'\b([0-9a-fA-F]+)\s+100\s+OBJECT\s+GLOBAL.*\bRESULTS\b', syms)
finished = re.search(r'\b([0-9a-fA-F]+)\s+0\s+NOTYPE\s+GLOBAL.*\bfinished\b', syms)
assert result and finished
addr, pc = int(result[1],16), int(finished[1],16)
with socket.socket() as listener:
    listener.bind(('127.0.0.1',0)); port=listener.getsockname()[1]
args = ['xvfb-run','-a','-s','-screen 0 1280x900x24',os.environ['DOLPHIN_BIN'], '--user', str(output/'dolphin-user'), '--batch','--video_backend=OGL','-C','Dolphin.Analytics.PermissionAsked=True','-C','Dolphin.Analytics.Enabled=False','-C','Dolphin.Interface.ConfirmStop=False','-C','Dolphin.Interface.AbortOnPanicAlert=True','-C',f'Dolphin.General.GDBPort={port}','-C','Dolphin.DSP.Backend=No Audio Output','--exec',str(dol)]
with (output/'dolphin.log').open('wb') as log:
    p=subprocess.Popen(args,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
    try:
        for _ in range(120):
            if p.poll() is not None: raise RuntimeError(f'Dolphin exited {p.returncode}')
            try: s=socket.create_connection(('127.0.0.1',port),timeout=.25); break
            except OSError: time.sleep(.25)
        else: raise TimeoutError('GDB port not available')
        with s:
            s.settimeout(10); pending=[]
            def byte():
                b=s.recv(1)
                if not b: raise EOFError('GDB closed')
                return b
            def after_start():
                d=b''
                while (b:=byte())!=b'#': d+=b
                assert int(byte()+byte(),16)==sum(d)%256
                s.sendall(b'+'); return d.decode()
            def packet(payload):
                d=payload.encode(); s.sendall(b'$'+d+b'#'+f'{sum(d)%256:02x}'.encode())
                ack=byte()
                if ack==b'$': pending.append(after_start())
                else: assert ack==b'+',ack
            def reply():
                if pending: return pending.pop(0)
                while byte()!=b'$': pass
                return after_start()
            packet('?'); initial=reply(); assert initial.startswith(('T','S')),initial
            packet('c')
            for _ in range(12):
                time.sleep(5); s.sendall(b'\x03'); stop=reply()
                while not stop: stop=reply()
                assert stop.startswith(('T','S')),stop
                packet(f'm{addr:x},64'); r=reply()
                while not r or r.startswith(('T','S')): r=reply()
                data=bytes.fromhex(r); assert len(data)==100
                if struct.unpack_from('>I',data)[0]==0x4d414432:
                    assert f'40:{pc:08x}' in stop.lower(),stop
                    break
                packet('c')
            else: raise TimeoutError('diagnostic did not complete')
    finally:
        try: os.killpg(p.pid,signal.SIGTERM)
        except ProcessLookupError: pass
        try: p.wait(timeout=3)
        except subprocess.TimeoutExpired: os.killpg(p.pid,signal.SIGKILL); p.wait()
(output/'target-results.bin').write_bytes(data)
magic,*words=struct.unpack('>25I',data)
fields='c_frames rust_frames header_fields header_mismatches subband_values subband_mismatches pcm_values pcm_mismatches c_recoverable_errors rust_recoverable_errors arena_bytes terminal_status'.split()
rows=[dict(zip(fields,words[:12])),dict(zip(fields,words[12:]))]
for row,n in zip(rows,[21,116]):
    assert row['c_frames']==row['rust_frames']==n
    assert row['header_fields']==6*n and row['header_mismatches']==0
    assert row['subband_values']==row['pcm_values']==2304*n
    assert row['c_recoverable_errors']==row['rust_recoverable_errors']==1
    assert row['terminal_status']==0 and row['arena_bytes']<8*1024*1024
if mode=='candidate':
    assert all(r['subband_mismatches']==r['pcm_mismatches']==0 for r in rows)
else:
    assert sum(r['subband_mismatches'] for r in rows)==126105
    assert sum(r['pcm_mismatches'] for r in rows)==302911
report={'mode':mode,'initial_stop':initial,'completion_stop':stop,'dol_sha256':hashlib.sha256(dol.read_bytes()).hexdigest(),'rows':rows}
(output/'report.json').write_text(json.dumps(report,indent=2)+'\n'); print(json.dumps(report,indent=2))
