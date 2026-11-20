# The proof's stand-in for github.com: the files under <root>, over TLS on 127.0.0.1:443.
# Usage: server.py <root> <certificate> <key>
#
# TLS runs through memory buffers, and the bytes it makes are sent to the socket here. macOS can
# refuse a send on loopback for want of buffer space (ENOBUFS) where a socket would otherwise
# block; through an ssl socket OpenSSL takes that error as a broken connection and the download
# ends (#3635). Here the same bytes go again once the socket takes them. Each file sent, each one
# the client stopped taking and each send that failed here is logged as "server: …", so the proof
# can tell its own server stopping from Sparkle's.
import errno, functools, http.server, io, select, ssl, sys, time


def say(line):
    print("server: " + line, file=sys.stderr, flush=True)


class Channel(io.RawIOBase):
    def __init__(self, sock, context):
        self.sock = sock
        self.incoming, self.outgoing = ssl.MemoryBIO(), ssl.MemoryBIO()
        self.tls = context.wrap_bio(self.incoming, self.outgoing, server_side=True)
        # The sends the kernel refused for a full buffer and took when sent again.
        self.full = 0
        self.until(self.tls.do_handshake)

    def readable(self):
        return True

    def writable(self):
        return True

    def send_out(self):
        view = memoryview(self.outgoing.read())
        while view:
            try:
                view = view[self.sock.send(view):]
            except OSError as e:
                if e.errno not in (errno.ENOBUFS, errno.EAGAIN, errno.EWOULDBLOCK):
                    raise
                self.full += 1
                # Until the socket can take more. ENOBUFS can come back with it writable, so a
                # moment passes before the next try, rather than a spin.
                select.select([], [self.sock], [], 1)
                if e.errno == errno.ENOBUFS:
                    time.sleep(0.005)

    # Runs a TLS step, sending what it makes and feeding it what it waits for.
    def until(self, step, *args):
        while True:
            try:
                result = step(*args)
            except ssl.SSLWantReadError:
                self.send_out()
                data = self.sock.recv(65536)
                if not data:
                    raise ConnectionResetError("the client closed the connection")
                self.incoming.write(data)
                continue
            self.send_out()
            return result

    def readinto(self, buffer):
        try:
            data = self.until(self.tls.read, len(buffer))
        except (ssl.SSLZeroReturnError, ConnectionResetError):
            return 0
        buffer[: len(data)] = data
        return len(data)

    def write(self, data):
        self.until(self.tls.write, data)
        return len(data)


def handler(context):
    class Handler(http.server.SimpleHTTPRequestHandler):
        def setup(self):
            self.connection = self.request
            self.channel = Channel(self.request, context)
            self.rfile = io.BufferedReader(self.channel, 65536)
            self.wfile = self.channel

        def copyfile(self, source, outputfile):
            sent = 0
            try:
                while True:
                    chunk = source.read(1 << 16)
                    if not chunk:
                        break
                    outputfile.write(chunk)
                    sent += len(chunk)
            except (BrokenPipeError, ConnectionResetError, ConnectionAbortedError) as e:
                say("the client stopped taking %s after %d bytes: %r" % (self.path, sent, e))
                raise
            except Exception as e:
                say("sending %s failed after %d bytes: %r" % (self.path, sent, e))
                raise
            say("sent %s: %d bytes, %d sends again after a full buffer" % (self.path, sent, self.channel.full))

        def finish(self):
            # Its close_notify, without waiting for the client's.
            try:
                self.channel.tls.unwrap()
            except ssl.SSLError:
                pass
            try:
                self.channel.send_out()
            except OSError:
                pass

    return Handler


def main():
    root, cert, key = sys.argv[1:4]
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(cert, key)
    Handler = handler(context)
    server = http.server.ThreadingHTTPServer(
        ("127.0.0.1", 443), functools.partial(Handler, directory=root)
    )
    server.serve_forever()


if __name__ == "__main__":
    main()
