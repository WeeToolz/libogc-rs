#!/usr/bin/env python3
"""Run an isolated SMB1 share for the Dolphin TinySMB probe.

Install impacket 0.13.1 in a virtual environment, create a disposable share
directory, then run this script with that directory as its only argument.
The server binds loopback port 14445 and uses the probe-only credentials.
The share contents are not deleted when the process exits.
"""

import argparse
from pathlib import Path

from impacket import ntlm, smb, smbserver
from impacket.nt_errors import STATUS_INVALID_HANDLE, STATUS_SUCCESS


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("share", type=Path, help="existing disposable share directory")
    args = parser.parse_args()
    if not args.share.is_dir():
        parser.error("share must be an existing directory")

    server = smbserver.SimpleSMBServer(listenAddress="127.0.0.1", listenPort=14445)
    server.addShare("OGC", str(args.share.resolve()))
    server.addCredential("probe", 0, ntlm.compute_lmhash("probe"), ntlm.compute_nthash("probe"))

    # libogc sends a legacy 0x04 buffer-format byte before ASCII OPEN_ANDX
    # filenames. Impacket otherwise interprets it as part of the filename.
    original_open = None

    def open_andx(connection, transport, command, packet):
        data = command["Data"]
        if not packet["Flags2"] & smb.SMB.FLAGS2_UNICODE and data[:1] == b"\x04":
            command["Data"] = data[1:]
        return original_open(connection, transport, command, packet)

    original_open = server.getServer().hookSmbCommand(smb.SMB.SMB_COM_OPEN_ANDX, open_andx)

    # Impacket 0.13.1 has no SMB_COM_FIND_CLOSE2 handler. Accept a well-formed
    # close request with a nonzero search ID so the production client can check
    # its response handling and invalidate its local search handle.
    def find_close(conn_id, smb_server, command, packet):
        if len(command["Parameters"]) < 2 or command["Parameters"][:2] == b"\x00\x00":
            return [smb.SMBCommand(smb.SMB.SMB_COM_FIND_CLOSE2)], None, STATUS_INVALID_HANDLE
        return [smb.SMBCommand(smb.SMB.SMB_COM_FIND_CLOSE2)], None, STATUS_SUCCESS

    server.getServer().hookSmbCommand(smb.SMB.SMB_COM_FIND_CLOSE2, find_close)
    print("SMB1 fixture listening on 127.0.0.1:14445", flush=True)
    server.start()


if __name__ == "__main__":
    main()
