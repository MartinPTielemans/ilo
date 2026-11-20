"""A stand-in for an ilo server's share routes, for share.sh (#680): the page at /s/<id> that the
server serves for every share, which names its own script and style, the share's sealed snapshot at
/s/<id>/data, 410 for a share that was unshared, and a page at /plain with the share's header at an
address that isn't a link's, which sets a cookie for the host. Each answer carries the headers the
server spec gives (#680's server comment). Each request is logged with whether it carried a cookie
or a referrer.

It seals the shares itself, as a device does (core/share_snapshot.h), in plain Python: AES-256-GCM
here is written out, as a run's Python has no cryptography package, and it's checked against the
NIST test vector it starts with.

usage: share-server.py <dir> <request log>
It writes <dir>/links, a line "<case> <path>#<key>" for each case: good, gone, wrong-key (the good
share's id with another key) and unknown (an id the server never had); and <dir>/secrets, what no
log line may hold: each link's id and key, its title and an address in it. Then it writes its port
on stdout.
"""
import base64, http.server, json, os, re, socketserver, sys, zlib

DIR, LOG = sys.argv[1:3]

# AES, FIPS 197.
SBOX = [0] * 256
_p = _q = 1
while True:
    _p = (_p ^ (_p << 1) ^ (0x1B if _p & 0x80 else 0)) & 0xFF
    _q ^= _q << 1
    _q ^= _q << 2
    _q ^= _q << 4
    _q &= 0xFF
    if _q & 0x80:
        _q ^= 0x09
    _x = _q ^ ((_q << 1) | (_q >> 7)) ^ ((_q << 2) | (_q >> 6)) ^ ((_q << 3) | (_q >> 5)) ^ (
        (_q << 4) | (_q >> 4))
    SBOX[_p] = (_x ^ 0x63) & 0xFF
    if _p == 1:
        break
SBOX[0] = 0x63


def xtime(b):
    return ((b << 1) ^ (0x1B if b & 0x80 else 0)) & 0xFF


def expand(key):
    words = [list(key[i:i + 4]) for i in range(0, 32, 4)]
    rcon = 1
    for i in range(8, 60):
        t = list(words[i - 1])
        if i % 8 == 0:
            t = [SBOX[b] for b in t[1:] + t[:1]]
            t[0] ^= rcon
            rcon = xtime(rcon)
        elif i % 8 == 4:
            t = [SBOX[b] for b in t]
        words.append([a ^ b for a, b in zip(words[i - 8], t)])
    return [sum(words[r * 4:r * 4 + 4], []) for r in range(15)]


def encrypt_block(keys, block):
    s = [a ^ b for a, b in zip(block, keys[0])]
    for r in range(1, 15):
        s = [SBOX[b] for b in s]
        s = [s[(i + 4 * (i % 4)) % 16] for i in range(16)]
        if r != 14:
            mixed = []
            for c in range(4):
                a = s[c * 4:c * 4 + 4]
                t = a[0] ^ a[1] ^ a[2] ^ a[3]
                mixed += [a[i] ^ t ^ xtime(a[i] ^ a[(i + 1) % 4]) for i in range(4)]
            s = mixed
        s = [a ^ b for a, b in zip(s, keys[r])]
    return bytes(s)


def gf_mult(x, y):
    z, v = 0, y
    for i in range(127, -1, -1):
        if (x >> i) & 1:
            z ^= v
        v = (v >> 1) ^ (0xE1 << 120) if v & 1 else v >> 1
    return z


def seal(key, iv, plain, aad):
    """AES-256-GCM, NIST SP 800-38D, with a 12-byte IV: the ciphertext and its 16-byte tag."""
    keys = expand(key)
    h = int.from_bytes(encrypt_block(keys, bytes(16)), "big")
    counter = lambda n: iv + n.to_bytes(4, "big")
    out = bytearray()
    for i in range(0, len(plain), 16):
        stream = encrypt_block(keys, counter(i // 16 + 2))
        out += bytes(a ^ b for a, b in zip(plain[i:i + 16], stream))
    g = 0
    for data in (aad, bytes(out)):
        padded = data + bytes(-len(data) % 16)
        for i in range(0, len(padded), 16):
            g = gf_mult(g ^ int.from_bytes(padded[i:i + 16], "big"), h)
    g = gf_mult(g ^ ((len(aad) * 8) << 64 | len(out) * 8), h)
    tag = bytes(a ^ b for a, b in zip(g.to_bytes(16, "big"), encrypt_block(keys, counter(1))))
    return bytes(out) + tag


# NIST's GCM test case 16: a 256-bit key, 60 bytes, 20 of additional data.
_k = bytes.fromhex("feffe9928665731c6d6a8f9467308308feffe9928665731c6d6a8f9467308308")
_iv = bytes.fromhex("cafebabefacedbaddecaf888")
_p16 = bytes.fromhex("d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a72"
                     "1c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b39")
_a = bytes.fromhex("feedfacedeadbeeffeedfacedeadbeefabaddad2")
assert seal(_k, _iv, _p16, _a)[-16:].hex() == "76fc6ece0f4e1768cddf8853bb2d551b", "AES-GCM is wrong"


def b64url(b):
    return base64.urlsafe_b64encode(b).rstrip(b"=").decode()


def make(title, items):
    """A share of |items|, sealed, its blob written: its id and key."""
    ident, key = os.urandom(16), os.urandom(32)
    snapshot = {"v": 1, "kind": "folder", "title": title, "made": 1795046400, "items": items}
    packer = zlib.compressobj(9, zlib.DEFLATED, -15)
    plain = packer.compress(json.dumps(snapshot).encode()) + packer.flush()
    iv = os.urandom(12)
    blob = b"\x01" + iv + seal(key, iv, plain, b"ilo share v1 " + b64url(ident).encode())
    with open(os.path.join(DIR, "blob-" + b64url(ident)), "wb") as f:
        f.write(blob)
    return b64url(ident), b64url(key)


secrets = []
links = []
good_id, good_key = make("Lisbon, March", [
    {"title": "Visit Lisboa", "url": "https://www.visitlisboa.example/en"},
    {"title": "Food", "items": [{"title": "Time Out Market", "url": "https://timeout.example/lx"}]},
    {"title": "Not a page", "url": "javascript:alert(1)"},
])
links.append("good /s/%s#%s" % (good_id, good_key))
secrets += [good_id, good_key, "Lisbon, March", "visitlisboa.example", "timeout.example"]
gone_id, gone_key = os.urandom(16), os.urandom(32)
links.append("gone /s/%s#%s" % (b64url(gone_id), b64url(gone_key)))
secrets += [b64url(gone_id), b64url(gone_key)]
GONE = {b64url(gone_id)}
wrong_key = b64url(os.urandom(32))
links.append("wrong-key /s/%s#%s" % (good_id, wrong_key))
unknown_id, unknown_key = b64url(os.urandom(16)), b64url(os.urandom(32))
links.append("unknown /s/%s#%s" % (unknown_id, unknown_key))
secrets += [wrong_key, unknown_id, unknown_key]
# Lines end in \n alone, as Windows' Python would end them in \r\n, and share.sh types a link
# whole: a \r in one is Enter in the command bar.
with open(os.path.join(DIR, "links"), "w", newline="\n") as f:
    f.write("\n".join(links) + "\n")
with open(os.path.join(DIR, "secrets"), "w", newline="\n") as f:
    f.write("\n".join(secrets) + "\n")

CSP = ("default-src 'none'; script-src 'self'; style-src 'self'; font-src 'self'; "
       "connect-src 'self'; img-src data:; base-uri 'none'; form-action 'self'; "
       "frame-ancestors 'none'")
PAGE = (b"<!doctype html><title>Shared from ilo</title><link rel=stylesheet href=/share.css>"
        b"<script src=/share.js></script><p>The server's own page.")
ID = r"[A-Za-z0-9_-]{22}"


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def send(self, code, body, kind, cookie=False):
        self.send_response(code)
        if cookie:
            self.send_header("Set-Cookie", "seen=1; SameSite=Lax; Path=/")
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Content-Security-Policy", CSP)
        self.send_header("Referrer-Policy", "no-referrer")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Cache-Control", "no-store")
        self.send_header("Ilo-Share", "v1")
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        path = self.path.split("?")[0]
        with open(LOG, "a", newline="\n") as log:
            log.write("%s cookie=%d referer=%d\n" % (
                path, "Cookie" in self.headers, "Referer" in self.headers))
        if path == "/plain" or re.fullmatch("/s/" + ID, path):
            return self.send(200, PAGE, "text/html; charset=utf-8", path == "/plain")
        m = re.fullmatch("/s/(" + ID + ")/data", path)
        if m and m.group(1) in GONE:
            return self.send(410, b"", "text/plain")
        blob = os.path.join(DIR, "blob-" + m.group(1)) if m else ""
        if m and os.path.exists(blob):
            with open(blob, "rb") as f:
                return self.send(200, f.read(), "application/octet-stream")
        self.send(404, b"", "text/plain")


# TCPServer, as site.sh binds: HTTPServer looks 127.0.0.1 up by name, 35 s on the hosted Mac.
server = socketserver.ThreadingTCPServer(("127.0.0.1", 0), Handler)
sys.stdout.buffer.write(b"%d\n" % server.server_address[1])
sys.stdout.close()
server.serve_forever()
