# A STUN server for the webrtc run, so a page's WebRTC can gather a server-reflexive candidate
# without the internet: it answers each Binding request with the address and port it came from, in
# an XOR-MAPPED-ADDRESS (RFC 8489). It listens on a free UDP port of the address it's given, which
# it writes to the file it's given, and logs "binding <address>:<port>" for each request it answers.
# Usage: stun.py <address> <port file>
import os
import socket
import struct
import sys

MAGIC = 0x2112A442
BINDING_REQUEST = 0x0001
BINDING_RESPONSE = 0x0101
XOR_MAPPED_ADDRESS = 0x0020

address, port_file = sys.argv[1:]
sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
sock.bind((address, 0))
with open(port_file + ".part", "w") as file:
    file.write(str(sock.getsockname()[1]))
# Whole, as the run reads it once it's there.
os.replace(port_file + ".part", port_file)
while True:
    data, (host, port) = sock.recvfrom(2048)
    if len(data) < 20:
        continue
    kind, _, magic = struct.unpack_from("!HHI", data)
    if kind != BINDING_REQUEST or magic != MAGIC:
        continue
    ip = struct.unpack("!I", socket.inet_aton(host))[0]
    mapped = struct.pack(
        "!HHBBHI", XOR_MAPPED_ADDRESS, 8, 0, 1, port ^ (MAGIC >> 16), ip ^ MAGIC
    )
    # The request's transaction id, after its type, length and magic cookie.
    header = struct.pack("!HHI", BINDING_RESPONSE, len(mapped), MAGIC) + data[8:20]
    sock.sendto(header + mapped, (host, port))
    print(f"binding {host}:{port}", flush=True)
