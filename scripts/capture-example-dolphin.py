"""Capture a Wii example through Dolphin GDB and convert its YUYV XFB into PNG screens.

The output includes each PNG, its raw XFB, the Dolphin log, and a JSON run record.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import socket
import signal
import subprocess
import time
import struct
import zlib
root = Path(__file__).resolve().parents[1]
from dolphin_capture_config import configure_capture_profile
from xfb_capture import summarize_yuyv_luma
parser = argparse.ArgumentParser(description='Capture screens from a Wii example DOL in Dolphin through the GDB XFB interface.')
parser.add_argument('--example', choices=('2048', 'letter-flip', 'easycap-patterns'), default='2048')
parser.add_argument('--label')
parser.add_argument('--dol', type=Path)
parser.add_argument('--elf', type=Path)
parser.add_argument('--dolphin', type=Path, default=Path('/Applications/Dolphin.app/Contents/MacOS/Dolphin'))
parser.add_argument('--warmup-seconds', type=int, help='Startup delay; defaults to 3 for EasyCap patterns, otherwise 12.')
parser.add_argument('--idle-seconds', type=int, help='Idle capture delay; defaults to 0 for EasyCap patterns, otherwise 20.')
parser.add_argument('--capture-audio', action='store_true', help='Save Dolphin DSP output as a WAV file.')
parser.add_argument('--ui-review', action='store_true', help='Capture example menus and gameplay review screens.')
parser.add_argument('--network-log', action='store_true', help='Record Dolphin IOS socket operations for connection diagnosis.')
parser.add_argument('--launch-services', action='store_true', help='Launch a macOS Dolphin app through open instead of spawning its executable directly.')
args = parser.parse_args()
if args.warmup_seconds is None:
    args.warmup_seconds = 3 if args.example == 'easycap-patterns' else 12
if args.idle_seconds is None:
    args.idle_seconds = 0 if args.example == 'easycap-patterns' else 20
label = args.label or args.example
if not label or any((c not in 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_' for c in label)):
    parser.error('label must use letters, digits, underscores, or hyphens')
if args.warmup_seconds < 0 or args.idle_seconds < 0:
    parser.error('capture delays must not be negative')
if args.example == 'letter-flip' and (not args.ui_review):
    parser.error('Letter Flip capture requires --ui-review')
binary_name = {'2048': 'autoplay_2048', 'letter-flip': 'letter-flip', 'easycap-patterns': 'easycap-patterns'}[args.example]
if args.example == 'easycap-patterns':
    default_dol = root / 'target/easycap-diagnostics/boot.dol'
    default_elf = root / 'target/xtask/custom-dol-build/powerpc-nintendo-none/release/easycap-patterns'
elif args.example == 'letter-flip':
    default_dol = root / 'target/xtask/custom-dols/examples-letter-flip.dol'
    default_elf = root / 'target/xtask/custom-dol-build/powerpc-nintendo-none/release/examples/letter-flip'
else:
    default_dol = root / 'target/xtask/examples' / f'{binary_name}.dol'
    default_elf = root / 'target/xtask/system-probe-build/powerpc-nintendo-none/release/examples' / binary_name
dol = (args.dol or default_dol).resolve()
elf = (args.elf or default_elf).resolve()
dol_hash = hashlib.sha256(dol.read_bytes()).hexdigest()
output = root / 'target' / ('dolphin-example-' + label)
profile = output / 'profile'
dump_root = output / 'dolphin-dump'
if args.capture_audio:
    (dump_root / 'Audio').mkdir(parents=True, exist_ok=True)
with socket.socket() as port_check:
    port_check.bind(('127.0.0.1', 55268))
configure_capture_profile(profile)
config = profile / 'Config'
dolphin_config = '[Core]\nCPUThread = False\nSIDevice0 = 6\nSIDevice1 = 0\nSIDevice2 = 0\nSIDevice3 = 0\nWiiSDCard = False\n[Interface]\nConfirmStop = False\nUsePanicHandlers = False\n[Input]\nBackgroundInput = True\n'
if args.capture_audio:
    dolphin_config += f'[General]\nDumpPath = {dump_root}\n[DSP]\nBackend = No Audio Output\nDumpAudio = True\nDumpAudioSilent = True\n'
(config / 'Dolphin.ini').write_text(dolphin_config)
(config / 'GCPadNew.ini').write_text('[GCPad1]\nDevice = Pipe/0/example\nButtons/A = `Button A`\nButtons/B = `Button B`\nButtons/X = `Button X`\nButtons/Y = `Button Y`\nButtons/Start = `Button START`\nD-Pad/Down = `Button D_DOWN`\nD-Pad/Up = `Button D_UP`\nD-Pad/Left = `Button D_LEFT`\nD-Pad/Right = `Button D_RIGHT`\n')
logger_config = '[Options]\nWriteToFile = True\nWriteToConsole = True\nVerbosity = ' + ('4' if args.network_log else '3') + '\n[Logs]\nVIDEO = True\nCOMMANDPROCESSOR = True\nOSREPORT = True\nCONTROLLERINTERFACE = True\n'
(config / 'Logger.ini').write_text(logger_config)
pipes = profile / 'Pipes'
pipes.mkdir(exist_ok=True)
pipe = pipes / 'example'
if pipe.exists():
    pipe.unlink()
os.mkfifo(pipe)
pipe_fd = os.open(pipe, os.O_RDWR | os.O_NONBLOCK)
symbols = subprocess.check_output(['/opt/homebrew/opt/llvm/bin/llvm-nm', '-nC', str(elf)], text=True)
xfb_symbol = next((int(line.split()[0], 16) for line in symbols.splitlines() if 'ogc_video::LAST_NEXT_FRAMEBUFFER ' in line))
ipc_wait_symbol = next((int(line.split()[0], 16) for line in symbols.splitlines() if line.endswith(' ogc_ipc::SYNC_WAIT_REQ')), None)
log_path = output / 'dolphin.log'
log = log_path.open('w')
dolphin_arguments = ['-b', '-u', str(profile), '-e', str(dol), '-C', 'Dolphin.General.GDBPort=55268']
profile_log = profile / 'Logs/dolphin.log'
if args.launch_services:
    dolphin_app = args.dolphin.resolve().parents[2]
    if not dolphin_app.name.endswith('.app'):
        parser.error('--launch-services requires a Dolphin executable inside a macOS app bundle')
    if profile_log.exists():
        profile_log.unlink()
    launch_command = ['open', '-W', '-n', '-a', str(dolphin_app), '--args', *dolphin_arguments]
else:
    launch_command = [str(args.dolphin), *dolphin_arguments]
proc = subprocess.Popen(launch_command, stdout=log, stderr=log)
dolphin_pid = None if args.launch_services else proc.pid

def launched_dolphin_pid():
    if not args.launch_services:
        return proc.pid
    processes = subprocess.check_output(['ps', '-ax', '-o', 'pid=', '-o', 'command='], text=True)
    matches = []
    for line in processes.splitlines():
        fields = line.strip().split(None, 1)
        if len(fields) == 2 and fields[1].startswith(str(args.dolphin) + ' ') and (f' -u {profile} ' in fields[1]):
            matches.append(int(fields[0]))
    if len(matches) > 1:
        raise RuntimeError('multiple Dolphin processes use this capture profile')
    return matches[0] if matches else None
sock = None
buffer = bytearray()

def packet(timeout=20):
    sock.settimeout(timeout)
    while True:
        start = buffer.find(b'$')
        end = buffer.find(b'#', start + 1) if start >= 0 else -1
        if end >= 0 and len(buffer) >= end + 3:
            raw = bytes(buffer[start + 1:end])
            checksum = int(bytes(buffer[end + 1:end + 3]), 16)
            del buffer[:end + 3]
            if sum(raw) & 255 != checksum:
                raise RuntimeError('invalid GDB checksum')
            sock.sendall(b'+')
            return raw.decode('ascii')
        data = sock.recv(65536)
        if not data:
            raise RuntimeError('Dolphin closed GDB connection; exit=' + str(proc.poll()))
        buffer.extend(data)

def command(payload):
    raw = payload.encode('ascii')
    sock.sendall(b'$' + raw + b'#' + f'{sum(raw) & 255:02x}'.encode())
    while True:
        reply = packet()
        if payload == '?' or (reply and (not reply.startswith(('T', 'S')))):
            return reply

def memory(address, count):
    reply = command(f'm{address:x},{count:x}')
    if len(reply) != count * 2:
        raise RuntimeError('bad memory reply: ' + reply[:100])
    return bytes.fromhex(reply)

def button(name, pressed):
    if name in ('UP', 'DOWN', 'LEFT', 'RIGHT'):
        name = 'D_' + name
    payload = f"{('PRESS' if pressed else 'RELEASE')} {name}\n".encode('ascii')
    if os.write(pipe_fd, payload) != len(payload):
        raise RuntimeError('incomplete Dolphin controller command')

def tap(name, hold_seconds=0.12, settle_seconds=0.18):
    button(name, True)
    run(hold_seconds)
    button(name, False)
    run(settle_seconds)

def run(seconds):
    sock.sendall(b'$c#63')
    time.sleep(seconds)
    sock.sendall(b'\x03')
    stop = packet()
    while not stop.startswith(('T', 'S')):
        stop = packet()
    print('stop', stop, flush=True)
    return stop

def capture(name):
    global capture_failure
    if args.network_log and ipc_wait_symbol is not None:
        request_pointer = int.from_bytes(memory(ipc_wait_symbol, 4), 'big')
        ipc = {'pending_request': request_pointer}
        if 2147483648 <= request_pointer < 2483027968:
            words = struct.unpack('>8I', memory(request_pointer, 32))
            ipc['request_words'] = list(words)
            if words[0] == 6 and words[3] == 1 and (words[5] == 4):
                ipc['accept_socket'] = int.from_bytes(memory(words[4] | 2147483648, 4), 'big')
        (output / (name + '.ipc.json')).write_text(json.dumps(ipc, indent=2) + '\n')
    pointer = int.from_bytes(memory(xfb_symbol, 4), 'big')
    cached = 2147483648 <= pointer < 2483027968
    uncached = 3221225472 <= pointer < 3556769792
    if not (cached or uncached):
        raise RuntimeError('invalid XFB pointer')
    address = pointer & 1073741823 | 2147483648
    length = 640 * 480 * 2
    xfb = b''.join((memory(address + offset, min(4096, length - offset)) for offset in range(0, length, 4096)))
    (output / (name + '.xfb')).write_bytes(xfb)
    try:
        summary = summarize_yuyv_luma(xfb, 640, 480)
    except ValueError as error:
        capture_failure = f'{name}: {error}'
        print('capture rejected:', capture_failure, flush=True)
        raise
    capture_summaries[name] = summary
    rgb = bytearray()
    for offset in range(0, len(xfb), 4):
        y0, u, y1, v = xfb[offset:offset + 4]
        for y in (y0, y1):
            c, d, e = (y - 16, u - 128, v - 128)
            channels = (298 * c + 409 * e + 128 >> 8, 298 * c - 100 * d - 208 * e + 128 >> 8, 298 * c + 516 * d + 128 >> 8)
            rgb.extend((max(0, min(255, value)) for value in channels))

    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
    rows = b''.join((b'\x00' + rgb[y * 1920:(y + 1) * 1920] for y in range(480)))
    png = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 640, 480, 8, 2, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(rows)) + chunk(b'IEND', b'')
    (output / (name + '.png')).write_bytes(png)
    print('capture', name, hex(pointer), summary, flush=True)
    return xfb

def round_complete_visible(xfb):
    """Detect the large white ROUND COMPLETE title in the game screen."""
    if len(xfb) != 640 * 480 * 2:
        raise ValueError('round-complete check requires a complete XFB')
    bright_pixels = 0
    for y in range(50, 84):
        row_start = y * 640 * 2
        for x_pair in range(50, 270):
            offset = row_start + x_pair * 4
            if xfb[offset] > 210:
                bright_pixels += 1
            if xfb[offset + 2] > 210:
                bright_pixels += 1
    return bright_pixels > 1000

def validate_easycap_patterns():
    global capture_failure
    reference_dir = root / 'target/easycap-diagnostics/references'
    spec = json.loads((reference_dir / 'identification.json').read_text())
    references = sorted(reference_dir.glob('*.yuyv'))
    if spec.get('version') != 1 or len(references) != spec['page_count']:
        capture_failure = 'Export current EasyCap source references before validation; see probes/easycap-patterns/README.md'
        raise RuntimeError(capture_failure)

    def fail(message):
        global capture_failure
        capture_failure = message
        raise RuntimeError(message)

    def decode_identity(xfb):
        decoded = []
        for parity, bands in enumerate(spec['bands']):
            words = []
            for top in bands:
                y = top + spec['band_height'] // 2 + parity
                word = 0
                for bit in range(32):
                    x = spec['bit_x'] + bit * spec['bit_width'] + spec['bit_width'] // 2
                    value = xfb[(y * 640 + x) * 2]
                    if value not in (spec['black_y'], spec['white_y']):
                        fail(f'Invalid source marker luma in parity {parity}, bit {bit}: {value}')
                    if xfb[((y ^ 1) * 640 + x) * 2] != spec['other_parity_y']:
                        fail(f'Invalid other-parity marker in parity {parity}, bit {bit}')
                    word = (word << 1) | int(value == spec['white_y'])
                words.append(word)
            if words[0] ^ words[1] != 0xffffffff:
                fail(f'Invalid inverted source ID in parity {parity}')
            decoded.append({'presentation_id': words[0], 'vi_retrace_snapshot': words[2]})
        if decoded[0] != decoded[1]:
            fail('Source XFB contains inconsistent even/odd row markers')
        return decoded[0]

    tap('START')  # Disable automatic cycling while checking each page.
    for page, reference in enumerate(references):
        if page:
            tap('A')
        actual = capture(reference.stem)
        expected = reference.read_bytes()
        if len(expected) != 640 * 480 * 2:
            capture_failure = f'invalid source reference size: {reference}'
            raise RuntimeError(capture_failure)
        if page == spec['page']:
            first = decode_identity(actual)
            run(0.25)
            second = decode_identity(capture('identification-live'))
            delta_id = (second['presentation_id'] - first['presentation_id']) & 0xffffffff
            delta_vi = (second['vi_retrace_snapshot'] - first['vi_retrace_snapshot']) & 0xffffffff
            if delta_id == 0 or delta_vi < delta_id:
                fail('Source presentation/retrace counters did not advance consistently')
            (output / 'identification.json').write_text(json.dumps({
                'first': first, 'second': second, 'presentation_delta': delta_id,
                'vi_retrace_delta': delta_vi,
            }, indent=2) + '\n')
        elif page != 5:  # The motion page changes with the live presentation counter.
            for y in (100, 200, 300, 380):
                start = y * 640 * 2
                if actual[start:start + 1280] != expected[start:start + 1280]:
                    capture_failure = f'EasyCap page {page}, row {y}: source XFB mismatch'
                    raise RuntimeError(capture_failure)
    tap('X')
    held = capture('held')
    run(2)
    if capture('held-after-two-seconds') != held:
        capture_failure = 'EasyCap Hold did not freeze the complete source frame'
        raise RuntimeError(capture_failure)

completed = False
capture_failure = None
capture_summaries = {}
started = time.monotonic()
try:
    for _ in range(100):
        try:
            sock = socket.create_connection(('127.0.0.1', 55268), timeout=0.5)
            break
        except OSError:
            if proc.poll() is not None:
                raise RuntimeError('Dolphin exited at startup')
            time.sleep(0.1)
    if sock is None:
        raise RuntimeError('Dolphin GDB port did not open')
    dolphin_pid = launched_dolphin_pid()
    if dolphin_pid is None:
        raise RuntimeError('could not identify the launched Dolphin process')
    print('initial', command('?'), flush=True)
    run(args.warmup_seconds)
    capture('startup')
    if args.example == 'easycap-patterns':
        validate_easycap_patterns()
    elif args.example == '2048':
        button('START', True)
        if args.ui_review:
            run(0.07)
            capture('menu-transition')
            run(0.43)
        else:
            run(0.5)
        button('START', False)
        run(1)
        capture('menu')
        if args.ui_review:
            tap('DOWN')
            tap('A')
            capture('how-to')
            tap('B')
            tap('DOWN')
            tap('A')
            capture('options')
            tap('B')
            tap('UP')
            tap('UP')
            tap('A')
            run(2)
        else:
            button('A', True)
            run(0.5)
            button('A', False)
            run(2)
    elif args.example == 'letter-flip':
        run(0.5)
        capture('title')
        tap('DOWN')
        capture('title-help-selected')
        tap('A')
        capture('help')
        tap('B')
        tap('DOWN')
        tap('DOWN')
        tap('A')
        capture('settings-game-top')
        for row in range(1, 6):
            tap('DOWN')
            if row == 3:
                tap('LEFT')
        capture('settings-game-bottom')
        tap('DOWN')
        capture('settings-sound-top')
        for _ in range(4):
            tap('DOWN')
        capture('settings-sound-bottom')
        tap('DOWN')
        capture('settings-controls-top')
        for row in range(12, 15):
            tap('DOWN')
            capture(f'settings-controls-row-{row}')
        tap('B')
        tap('A', settle_seconds=0.5)
    else:
        button('RIGHT', True)
        run(0.5)
        button('RIGHT', False)
        run(2)
    capture('gameplay')
    if args.example == 'letter-flip' and args.ui_review:
        tap('A')
        capture('feedback')
        tap('START')
        capture('paused')
        tap('A')
        capture('feedback-resumed')
        run(4)
        round_complete = False
        for attempt in range(12):
            tap('A')
            run(6)
            frame = capture(f'round-progress-{attempt + 1}')
            if round_complete_visible(frame):
                from shutil import copyfile
                copyfile(output / f'round-progress-{attempt + 1}.png', output / 'round-complete.png')
                copyfile(output / f'round-progress-{attempt + 1}.xfb', output / 'round-complete.xfb')
                capture_summaries['round-complete'] = capture_summaries[f'round-progress-{attempt + 1}']
                round_complete = True
                break
        if not round_complete:
            raise RuntimeError('the Letter Flip round-complete screen did not appear within 12 answer attempts')
    if args.example == '2048' and args.ui_review:
        for direction in ('LEFT', 'UP', 'RIGHT', 'DOWN'):
            button(direction, True)
            run(0.07)
            button(direction, False)
            capture('move-' + direction.lower())
            run(0.18)
        tap('START')
        capture('pause')
        tap('DOWN')
        tap('DOWN')
        tap('A')
        capture('pause-options')
        tap('B')
        tap('UP')
        tap('A')
        capture('restart-confirmation')
        tap('B')
        tap('B')
        run(1)
        capture('gameplay-resumed')
    run(args.idle_seconds)
    capture('idle')
    completed = True
finally:
    cleanup_started_monotonic = time.monotonic()
    if sock:
        sock.close()
    dolphin_pid = dolphin_pid or launched_dolphin_pid()
    if dolphin_pid is not None:
        try:
            os.kill(dolphin_pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    else:
        proc.terminate()
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        if dolphin_pid is not None:
            try:
                os.kill(dolphin_pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        proc.kill()
        proc.wait()
    os.close(pipe_fd)
    log.close()
    if args.launch_services and profile_log.exists():
        log_path.write_text(profile_log.read_text(errors='replace'))
    data = log_path.read_text(errors='replace')
    online_reports = [line for line in data.splitlines() if 'application online:' in line or 'application host probe:' in line]
    print('Unknown Opcode occurrences:', data.count('Unknown Opcode'), flush=True)
    result = {'example': args.example, 'launch_services': args.launch_services, 'dol': str(dol), 'fifo_error_count': data.count('Unknown Opcode'), 'dolphin_exit': None if args.launch_services else proc.returncode, 'launcher_exit': proc.returncode if args.launch_services else None, 'completed': completed, 'cleanup_started_monotonic': cleanup_started_monotonic, 'dol_sha256': dol_hash, 'elapsed_seconds': round(time.monotonic() - started, 2), 'warmup_seconds': args.warmup_seconds, 'idle_seconds': args.idle_seconds, 'visual_review_required': True, 'capture_failure': capture_failure, 'online_stage_reports': online_reports, 'audio_files': [str(path) for path in dump_root.rglob('*.wav')] if args.capture_audio else []}
    (output / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
if 'Unknown Opcode' in data:
    raise SystemExit('FAIL: Dolphin reported an invalid GX opcode')
print('Capture complete. Review the PNG files for the requested screen transitions.', flush=True)
