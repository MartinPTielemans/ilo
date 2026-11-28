// A minimal RFC 6455 client, hand-rolled over java.net.Socket: android.jar carries no
// java.net.http at any API level, and the relay speaks only text frames over one connection.

package app.ilo.browser.android.sync

import java.io.InputStream
import java.io.OutputStream
import java.net.Socket
import java.security.MessageDigest
import java.security.SecureRandom
import java.util.Base64
import javax.net.ssl.HostnameVerifier
import javax.net.ssl.HttpsURLConnection
import javax.net.ssl.SSLSocket
import javax.net.ssl.SSLSocketFactory

/** One WebSocket frame, as bytes either way. Text carries the relay's JSON; the rest is the
 * protocol's own plumbing. */
class WsFrame(val opcode: Int, val payload: ByteArray) {

    fun serialized(): ByteArray {
        val out = ArrayList<Byte>(2 + payload.size + 4)
        // FIN always set: the relay's frames are small enough that no continuation follows.
        out.add((0x80 or opcode).toByte())
        when {
            payload.size < 126 -> out.add((0x80 or payload.size).toByte())
            payload.size < 65536 -> {
                out.add((0x80 or 126).toByte())
                out.add((payload.size shr 8).toByte())
                out.add(payload.size.toByte())
            }
            else -> {
                out.add((0x80 or 127).toByte())
                for (shift in 56 downTo 0 step 8) out.add((payload.size.toLong() shr shift).toByte())
            }
        }
        // Client frames are masked, as the protocol demands; the mask is fresh each frame.
        val mask = ByteArray(4).also { random.nextBytes(it) }
        for (b in mask) out.add(b)
        for (i in payload.indices) out.add((payload[i].toInt() xor mask[i and 3].toInt()).toByte())
        return out.toByteArray()
    }

    companion object {
        const val TEXT = 0x1
        const val CLOSE = 0x8
        const val PING = 0x9
        const val PONG = 0xA

        /** Server frames come unmasked; a masked one isn't from a server, so it's refused. */
        fun parse(bytes: ByteArray, into: ArrayList<WsFrame>): Boolean {
            var at = 0
            while (at < bytes.size) {
                val first = bytes[at].toInt() and 0xff
                if (first and 0x80 == 0) return false
                val opcode = first and 0x0f
                if (at + 1 >= bytes.size) return true
                val second = bytes[at + 1].toInt() and 0xff
                if (second and 0x80 != 0) return false
                var length = (second and 0x7f).toLong()
                var header = 2
                if (length == 126L) {
                    if (at + 4 > bytes.size) return true
                    length = ((bytes[at + 2].toInt() and 0xff).toLong() shl 8) or
                        (bytes[at + 3].toInt() and 0xff).toLong()
                    header = 4
                } else if (length == 127L) {
                    if (at + 10 > bytes.size) return true
                    length = 0
                    for (i in 0 until 8) {
                        length = length shl 8 or (bytes[at + 2 + i].toLong() and 0xff)
                    }
                    header = 10
                }
                if (length > MAX_FRAME) return false
                val end = at + header + length.toInt()
                if (end > bytes.size) return true
                val payload = bytes.copyOfRange(at + header, end)
                val control = first and 0x0f >= 0x8
                if (control && payload.size > 125) return false
                into.add(WsFrame(opcode, payload))
                at = end
            }
            return true
        }
    }
}

private const val MAX_FRAME = 1024 * 1024
private val random = SecureRandom()

/** The relay's connection: handshake, text frames in and out, ping answered, close on drop. */
class WsSocket(
    private val url: String,
    private val authorization: String,
    /** Called with each text frame's bytes; runs on the reading thread. */
    private val onText: (ByteArray) -> Unit,
    /** The close the server sent, or the HTTP status when it refused the upgrade; null when
     * the connection just broke. */
    private val onClosed: (code: Int?) -> Unit,
    /** The layered factory validates the chain but not the hostname, so this check is ours. */
    internal val hostnameVerifier: HostnameVerifier =
        HttpsURLConnection.getDefaultHostnameVerifier(),
    /** Injectable so a test TLS peer with a self-signed certificate can complete a handshake. */
    internal val sslFactory: SSLSocketFactory =
        SSLSocketFactory.getDefault() as SSLSocketFactory,
) {
    private var socket: Socket? = null
    private var out: OutputStream? = null
    private var reader: Thread? = null
    @Volatile private var closedByUs = false

    fun connect(): Boolean {
        // Nothing throws out of here: the caller runs this bare inside a coroutine, and an
        // unhandled exception there kills the app, so every failure goes through onClosed.
        var sock: Socket? = null
        try {
            val uri = java.net.URI(url)
            val secure = when (uri.scheme?.lowercase()) {
                "wss", "https" -> true
                "ws", "http" -> false
                else -> return false
            }
            val port = if (uri.port > 0) uri.port else if (secure) 443 else 80
            val raw = Socket(uri.host, port)
            sock = raw
            val connected = if (secure) {
                // The layered overload takes the socket the tunnel runs on, so no second
                // connection.
                val ssl = sslFactory.createSocket(raw, uri.host, port, true) as SSLSocket
                sock = ssl
                // The handshake otherwise starts on first IO, and the session must exist to
                // check the peer's certificate against the name we meant to reach.
                ssl.startHandshake()
                if (!hostnameVerifier.verify(uri.host, ssl.session)) {
                    ssl.close()
                    onClosed(null)
                    return false
                }
                ssl
            } else raw
            connected.tcpNoDelay = true
            val key = ByteArray(16).also { random.nextBytes(it) }.base64()
            val path = (uri.rawPath.ifEmpty { "/" }) +
                (uri.rawQuery?.let { "?$it" } ?: "")
            val request = "GET $path HTTP/1.1\r\n" +
                "Host: ${uri.host}" + (if (uri.port > 0) ":${uri.port}" else "") + "\r\n" +
                "Upgrade: websocket\r\n" +
                "Connection: Upgrade\r\n" +
                "Sec-WebSocket-Key: $key\r\n" +
                "Sec-WebSocket-Version: 13\r\n" +
                "Authorization: $authorization\r\n" +
                "\r\n"
            val output = connected.getOutputStream()
            output.write(request.encodeToByteArray())
            output.flush()
            val input = connected.getInputStream()
            val status = readHttpStatus(input) ?: run {
                connected.close(); onClosed(null); return false
            }
            if (status != 101) {
                connected.close()
                onClosed(status)
                return false
            }
            // RFC 6455 4.1: the reply must carry the Accept our key hashes to, or the
            // peer isn't a WebSocket server and the connection is closed.
            val headers = skipToBody(input) ?: run {
                connected.close(); onClosed(null); return false
            }
            if (headers["sec-websocket-accept"] != acceptFor(key)) {
                connected.close(); onClosed(null); return false
            }
            socket = connected
            out = output
            reader = Thread { readLoop(input) }.also { it.start() }
            return true
        } catch (_: Exception) {
            try {
                sock?.close()
            } catch (_: Exception) {}
            onClosed(null)
            return false
        }
    }

    private fun readHttpStatus(input: InputStream): Int? {
        val line = readLine(input) ?: return null
        val parts = line.split(" ")
        return if (parts.size >= 2) parts[1].toIntOrNull() else null
    }

    /** The response headers up to the blank line, lower-cased by name; anything after is
     * already frame bytes. */
    private fun skipToBody(input: InputStream): Map<String, String>? {
        val headers = HashMap<String, String>()
        while (true) {
            val line = readLine(input) ?: return null
            if (line.isEmpty()) return headers
            val colon = line.indexOf(':')
            if (colon > 0) {
                headers[line.substring(0, colon).lowercase().trim()] =
                    line.substring(colon + 1).trim()
            }
        }
    }

    /** The Sec-WebSocket-Accept the protocol derives from the key we sent. */
    internal fun acceptFor(key: String): String =
        Base64.getEncoder().encodeToString(
            MessageDigest.getInstance("SHA-1")
                .digest((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encodeToByteArray()))

    private fun readLine(input: InputStream): String? {
        val bytes = ArrayList<Byte>()
        while (true) {
            val b = input.read()
            if (b == -1) return if (bytes.isEmpty()) null else null
            if (b == 0x0d) {
                if (input.read() == 0x0a) return bytes.toByteArray().decodeToString()
                return null
            }
            if (b == 0x0a) return bytes.toByteArray().decodeToString()
            bytes.add(b.toByte())
            if (bytes.size > 8192) return null
        }
    }

    private fun readLoop(input: InputStream) {
        val buffer = ByteArray(8192)
        val pending = ArrayList<Byte>()
        val frames = ArrayList<WsFrame>()
        try {
            while (true) {
                val read = input.read(buffer)
                if (read == -1) break
                for (i in 0 until read) pending.add(buffer[i])
                val bytes = pending.toByteArray()
                frames.clear()
                if (!WsFrame.parse(bytes, frames)) break
                if (frames.isEmpty()) continue
                val exact = consumedFor(bytes, frames)
                // Drop what parsed, keep the tail for the next read.
                val tail = bytes.copyOfRange(exact, bytes.size)
                pending.clear()
                for (b in tail) pending.add(b)
                for (frame in frames) when (frame.opcode) {
                    WsFrame.TEXT -> onText(frame.payload)
                    WsFrame.PING -> send(WsFrame(WsFrame.PONG, frame.payload))
                    WsFrame.PONG -> {}
                    WsFrame.CLOSE -> {
                        val code = if (frame.payload.size >= 2)
                            ((frame.payload[0].toInt() and 0xff) shl 8) or
                                (frame.payload[1].toInt() and 0xff) else 1005
                        closedByUs = true
                        try {
                            send(WsFrame(WsFrame.CLOSE, frame.payload))
                        } catch (_: Exception) {}
                        close()
                        onClosed(code)
                        return
                    }
                    else -> {}
                }
            }
        } catch (_: Exception) {
        }
        close()
        if (!closedByUs) onClosed(null)
    }

    /** How many bytes the parsed frames took, including any extended-length headers. */
    private fun consumedFor(bytes: ByteArray, frames: List<WsFrame>): Int {
        var at = 0
        for (frame in frames) {
            var header = 2
            val length = (bytes[at + 1].toInt() and 0x7f).toLong()
            if (length == 126L) header = 4 else if (length == 127L) header = 10
            at += header + frame.payload.size
        }
        return at
    }

    fun send(frame: WsFrame): Boolean = try {
        val output = out ?: return false
        output.write(frame.serialized())
        output.flush()
        true
    } catch (_: Exception) {
        false
    }

    fun sendText(text: String): Boolean = send(WsFrame(WsFrame.TEXT, text.encodeToByteArray()))

    fun close() {
        closedByUs = true
        try {
            socket?.close()
        } catch (_: Exception) {}
    }
}

/** The relay's socket as [AccountRelay.RelaySocket] takes it, connecting on its own thread so
 * the caller's never waits on the network. */
class WsRelaySocket(
    url: String,
    authorization: String,
    onText: (ByteArray) -> Unit,
    onClosed: (code: Int?) -> Unit,
) : AccountRelay.RelaySocket {
    private val socket = WsSocket(url, authorization, onText, onClosed)
    @Volatile private var connected = false

    override fun connect(): Boolean {
        // The handshake blocks, so it runs where the caller's thread can wait for its verdict.
        connected = try {
            socket.connect()
        } catch (_: Exception) {
            false
        }
        return connected
    }

    override fun sendText(text: String): Boolean = connected && socket.sendText(text)

    override fun close() = socket.close()
}
