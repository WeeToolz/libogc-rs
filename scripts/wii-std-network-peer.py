#!/usr/bin/env python3
"""Provide real host-side TCP and UDP peers for the Wii readiness probe."""
import argparse
import json
import socket
import socketserver
import sys
import threading

PAYLOAD = b"WII_STD_TCP_51A7"
REPLY = bytes((0x51, 0xA7, 0x20, 0x26))


class TcpPeer(socketserver.StreamRequestHandler):
    def handle(self):
        try:
            request = self.rfile.readline(128).decode("ascii", "strict").strip().split()
            if len(request) != 2:
                raise ValueError("probe sent an invalid listener address")
            address, port = request[0], int(request[1])
            print(f"TCP request targets {address}:{port}", file=sys.stderr, flush=True)
            with socket.create_connection((address, port), timeout=10) as peer:
                print("TCP callback connected", file=sys.stderr, flush=True)
                peer.settimeout(10)
                peer.sendall(PAYLOAD)
                received = bytearray()
                while len(received) < len(REPLY):
                    chunk = peer.recv(len(REPLY) - len(received))
                    if not chunk:
                        raise ConnectionError("Wii closed before sending its TCP reply")
                    received.extend(chunk)
                self.wfile.write(b"\x00" + received)
                print("TCP callback exchanged payload", file=sys.stderr, flush=True)
        except Exception as error:
            detail = str(error).encode("ascii", "replace")
            print(f"TCP peer failed: {error}", file=sys.stderr, flush=True)
            self.wfile.write(b"\x01" + detail + b"\n")


class TcpServer(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


def udp_echo(sock, stopped):
    sock.settimeout(0.25)
    while not stopped.is_set():
        try:
            data, address = sock.recvfrom(65507)
        except socket.timeout:
            continue
        except OSError:
            return
        print(f"UDP received {len(data)} bytes from {address}", file=sys.stderr, flush=True)
        try:
            sent = sock.sendto(data, address)
            if sent != len(data):
                print(f"UDP short echo: sent {sent} of {len(data)} bytes", file=sys.stderr, flush=True)
        except OSError as error:
            print(f"UDP echo to {address} failed: {error}", file=sys.stderr, flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--host-ip", required=True)
    args = parser.parse_args()
    tcp = TcpServer((args.host_ip, 0), TcpPeer)
    udp = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    udp.bind((args.host_ip, 0))
    stopped = threading.Event()
    threading.Thread(target=tcp.serve_forever, daemon=True).start()
    threading.Thread(target=udp_echo, args=(udp, stopped), daemon=True).start()
    print(
        json.dumps({"tcp_port": tcp.server_address[1], "udp_port": udp.getsockname()[1]}),
        flush=True,
    )
    try:
        threading.Event().wait()
    finally:
        stopped.set()
        tcp.shutdown()
        tcp.server_close()
        udp.close()


if __name__ == "__main__":
    main()
