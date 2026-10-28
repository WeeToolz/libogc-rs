import json
from contextlib import ExitStack
import os
from pathlib import Path
import selectors
import socket
import subprocess
import sys
import time
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "wii-std-network-peer.py"
TIMEOUT = 3


def read_startup_line(stream, timeout=TIMEOUT):
    deadline = time.monotonic() + timeout
    line = bytearray()
    with selectors.DefaultSelector() as selector:
        selector.register(stream, selectors.EVENT_READ)
        while not line.endswith(b"\n"):
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not selector.select(remaining):
                raise TimeoutError("network peer did not publish startup ports in time")
            byte = os.read(stream.fileno(), 1)
            if not byte:
                raise EOFError("network peer exited before publishing startup ports")
            line.extend(byte)
    return line.decode("utf-8")


def receive_exact(sock, length):
    result = bytearray()
    while len(result) < length:
        chunk = sock.recv(length - len(result))
        if not chunk:
            raise EOFError(f"connection closed after {len(result)} of {length} bytes")
        result.extend(chunk)
    return bytes(result)


def stop_process(process):
    if process.poll() is None:
        process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)
    process.stdout.close()


class NetworkPeerTests(unittest.TestCase):
    def test_closed_connection_fails_without_looping(self):
        reader, writer = socket.socketpair()
        with reader, writer:
            writer.close()
            reader.settimeout(TIMEOUT)
            with self.assertRaisesRegex(EOFError, "connection closed after 0 of 1 bytes"):
                receive_exact(reader, 1)

    def test_startup_read_has_a_deadline(self):
        process = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(10)"], stdout=subprocess.PIPE, text=True)
        try:
            with self.assertRaisesRegex(TimeoutError, "did not publish"):
                read_startup_line(process.stdout, 0.05)
        finally:
            stop_process(process)

    def test_partial_startup_line_has_a_deadline(self):
        process = subprocess.Popen([sys.executable, "-c", "import sys,time; sys.stdout.write('partial'); sys.stdout.flush(); time.sleep(10)"], stdout=subprocess.PIPE, text=True)
        try:
            with self.assertRaisesRegex(TimeoutError, "did not publish"):
                read_startup_line(process.stdout, 0.1)
        finally:
            stop_process(process)

    def test_continuous_startup_output_without_newline_has_a_deadline(self):
        process = subprocess.Popen(
            [sys.executable, "-c", "import sys;\nwhile True: sys.stdout.write('x'); sys.stdout.flush()"],
            stdout=subprocess.PIPE,
            text=True,
        )
        try:
            with self.assertRaisesRegex(TimeoutError, "did not publish"):
                read_startup_line(process.stdout, 0.05)
        finally:
            stop_process(process)

    def test_tcp_coordinator_and_udp_echo_use_real_sockets(self):
        process = subprocess.Popen(
            [sys.executable, str(SCRIPT), "--host-ip", "127.0.0.1"],
            stdout=subprocess.PIPE,
            text=True,
        )
        try:
            ports = json.loads(read_startup_line(process.stdout))
            # Wii startup can take longer than the peer's idle poll interval.
            time.sleep(0.75)
            with ExitStack() as sockets:
                listener = sockets.enter_context(socket.socket())
                listener.settimeout(TIMEOUT)
                listener.bind(("127.0.0.1", 0))
                listener.listen()
                control = sockets.enter_context(socket.create_connection(("127.0.0.1", ports["tcp_port"]), timeout=TIMEOUT))
                control.settimeout(TIMEOUT)
                control.sendall(f"127.0.0.1 {listener.getsockname()[1]}\n".encode())
                peer, _ = listener.accept()
                sockets.enter_context(peer)
                peer.settimeout(TIMEOUT)
                self.assertEqual(receive_exact(peer, len(b"WII_STD_TCP_51A7")), b"WII_STD_TCP_51A7")
                peer.sendall(bytes((0x51, 0xA7, 0x20, 0x26)))
                self.assertEqual(receive_exact(control, 5), bytes((0, 0x51, 0xA7, 0x20, 0x26)))
                udp = sockets.enter_context(socket.socket(socket.AF_INET, socket.SOCK_DGRAM))
                udp.settimeout(TIMEOUT)
                for payload in (b"51a7-data", b""):
                    udp.sendto(payload, ("127.0.0.1", ports["udp_port"]))
                    self.assertEqual(udp.recvfrom(64)[0], payload)
        finally:
            stop_process(process)


if __name__ == "__main__":
    unittest.main()
