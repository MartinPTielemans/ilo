package app.ilo.browser.android.sync

import java.io.InputStream
import java.io.OutputStream
import java.net.ServerSocket
import java.security.KeyStore
import java.security.MessageDigest
import java.security.SecureRandom
import java.security.cert.X509Certificate
import java.util.Base64
import javax.net.ssl.HostnameVerifier
import javax.net.ssl.KeyManagerFactory
import javax.net.ssl.SSLContext
import javax.net.ssl.SSLSocket
import javax.net.ssl.SSLSocketFactory
import javax.net.ssl.X509TrustManager
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** End-to-end socket tests against a loopback server: handshake headers, frame exchange,
 * slow delivery, TLS hostname refusal and the Accept check. */
class WsLoopbackTest {

    private val guid = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"

    @Test
    fun acceptMatchesTheRfcSampleVector() {
        // RFC 6455 page 53: this key and GUID must give exactly this Accept, which pins the
        // digest to SHA-1 and the GUID's spelling.
        assertEquals(
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=",
            WsSocket("ws://x/", "", {}, {}).acceptFor("dGhlIHNhbXBsZSBub25jZQ=="))
    }

    private fun acceptFor(key: String): String = Base64.getEncoder().encodeToString(
        MessageDigest.getInstance("SHA-1").digest((key + guid).encodeToByteArray()))

    /** Reads one CRLF line from the raw stream. */
    private fun line(input: InputStream): String {
        val bytes = ArrayList<Byte>()
        while (true) {
            val b = input.read()
            if (b == 0x0a) return bytes.toByteArray().decodeToString().trim()
            bytes.add(b.toByte())
        }
    }

    /** Reads the request up to its blank line and hands back the headers by name. */
    private fun readRequest(input: InputStream): Map<String, String> {
        var request = ""
        while (true) {
            val l = line(input)
            if (l.isEmpty()) return parseHeaders(request)
            request += "$l\n"
        }
    }

    private fun parseHeaders(text: String): Map<String, String> {
        val lines = text.split("\n").filter { it.contains(":") }
        val map = HashMap<String, String>()
        for (l in lines) {
            val colon = l.indexOf(':')
            map[l.substring(0, colon).lowercase().trim()] = l.substring(colon + 1).trim()
        }
        return map
    }

    private fun writeHandshake(
        output: OutputStream, accept: String, status: Int = 101, extra: String = "",
    ) {
        output.write(
            ("HTTP/1.1 $status Switching Protocols\r\n" +
                "Upgrade: websocket\r\n" +
                "Connection: Upgrade\r\n" +
                "Sec-WebSocket-Accept: $accept$extra\r\n" +
                "\r\n").encodeToByteArray())
        output.flush()
    }

    /** One server frame: FIN + opcode, unmasked server form. */
    private fun serverFrame(opcode: Int, payload: ByteArray): ByteArray {
        val out = ArrayList<Byte>(2 + payload.size)
        out.add((0x80 or opcode).toByte())
        if (payload.size < 126) {
            out.add(payload.size.toByte())
        } else {
            out.add(126.toByte())
            out.add((payload.size shr 8).toByte())
            out.add(payload.size.toByte())
        }
        for (b in payload) out.add(b)
        return out.toByteArray()
    }

    /** Strips the mask of a client frame and returns its payload. */
    private fun unmask(frame: ByteArray): ByteArray {
        var at = 0
        val first = frame[0].toInt() and 0xff
        val opcode = first and 0x0f
        assertTrue("fin set", first and 0x80 != 0)
        val second = frame[1].toInt() and 0xff
        assertTrue("mask bit set", second and 0x80 != 0)
        var length = (second and 0x7f).toLong()
        at = 2
        if (length == 126L) {
            length = ((frame[2].toInt() and 0xff).toLong() shl 8) or (frame[3].toInt() and 0xff).toLong()
            at = 4
        }
        val mask = frame.copyOfRange(at, at + 4)
        at += 4
        val payload = ByteArray(length.toInt())
        for (i in 0 until length.toInt()) {
            payload[i] = (frame[at + i].toInt() xor mask[i and 3].toInt()).toByte()
        }
        assertEquals(WsFrame.TEXT, opcode)
        return payload
    }

    @Test
    fun plaintextHandshakeAndFrameExchange() {
        val server = ServerSocket(0)
        val received = ArrayList<String>()
        val texts = ArrayList<String>()
        var headers: Map<String, String>? = null
        val accept = Thread {
            val peer = server.accept()
            headers = readRequest(peer.getInputStream())
            writeHandshake(peer.getOutputStream(), acceptFor(headers!!["sec-websocket-key"]!!))
            peer.getOutputStream().write(serverFrame(WsFrame.TEXT, "\"hello\"".encodeToByteArray()))
            peer.getOutputStream().flush()
            val input = peer.getInputStream()
            val header = ByteArray(2)
            readFully(input, header)
            val length = (header[1].toInt() and 0x7f)
            val mask = ByteArray(4)
            readFully(input, mask)
            val masked = ByteArray(length)
            readFully(input, masked)
            received.add(String(unmask(header + mask + masked)))
            peer.close()
            server.close()
        }
        accept.start()
        var closed: Int? = -1
        val socket = WsSocket(
            "ws://127.0.0.1:${server.localPort}/relay", "Bearer secret-token",
            { bytes -> texts.add(bytes.decodeToString()) }, { code -> closed = code })
        assertTrue(socket.connect())
        socket.sendText("reply")
        // The frame arrives on the reader thread; poll until it does, then close.
        val deadline = System.currentTimeMillis() + 5000
        while (texts.isEmpty() && System.currentTimeMillis() < deadline) Thread.sleep(10)
        socket.close()
        accept.join(5000)
        val h = headers!!
        assertEquals("Bearer secret-token", h["authorization"])
        assertEquals("websocket", h["upgrade"]?.lowercase())
        assertEquals("Upgrade", h["connection"])
        assertEquals("13", h["sec-websocket-version"])
        assertTrue(h["host"]!!.contains("127.0.0.1"))
        assertEquals(1, texts.size)
        assertEquals("\"hello\"", texts[0])
        assertEquals(listOf("reply"), received)
    }

    private fun readFully(input: InputStream, into: ByteArray) {
        var at = 0
        while (at < into.size) {
            val n = input.read(into, at, into.size - at)
            if (n == -1) throw java.io.EOFException()
            at += n
        }
    }

    @Test
    fun slowDeliveryAcrossWrites() {
        val payload = ByteArray(300) { (it % 251).toByte() }
        val server = ServerSocket(0)
        val texts = ArrayList<ByteArray>()
        val thread = Thread {
            val peer = server.accept()
            val input = peer.getInputStream()
            val request = StringBuilder()
            var key = ""
            while (true) {
                val l = line(input)
                if (l.isEmpty()) break
                if (l.startsWith("Sec-WebSocket-Key:", ignoreCase = true)) {
                    key = l.substringAfter(":").trim()
                }
                request.append(l)
            }
            writeHandshake(peer.getOutputStream(), acceptFor(key))
            val frame = serverFrame(WsFrame.TEXT, payload)
            val output = peer.getOutputStream()
            // Split the frame: one header byte, then the rest of the header, then halves of
            // the payload, with the reader left waiting between each piece.
            output.write(frame.copyOfRange(0, 1)); output.flush()
            output.write(frame.copyOfRange(1, 4)); output.flush()
            output.write(frame.copyOfRange(4, 4 + payload.size / 2)); output.flush()
            Thread.sleep(100)
            output.write(frame.copyOfRange(4 + payload.size / 2, frame.size)); output.flush()
            peer.getInputStream().read()
            peer.close()
            server.close()
        }
        thread.start()
        var closed: Int? = -1
        val socket = WsSocket(
            "ws://127.0.0.1:${server.localPort}/relay", "Bearer x",
            { bytes -> texts.add(bytes) }, { code -> closed = code })
        assertTrue(socket.connect())
        // The frame arrives only after its last piece; poll until it does.
        val deadline = System.currentTimeMillis() + 5000
        while (texts.isEmpty() && System.currentTimeMillis() < deadline) Thread.sleep(10)
        socket.close()
        thread.join(5000)
        assertEquals(1, texts.size)
        assertTrue(texts[0].contentEquals(payload))
    }

    @Test
    fun wrongAcceptRefused() {
        val server = ServerSocket(0)
        val thread = Thread {
            val peer = server.accept()
            readRequest(peer.getInputStream())
            writeHandshake(peer.getOutputStream(), "not-the-accept")
            peer.getInputStream().read()
            peer.close()
            server.close()
        }
        thread.start()
        var closed: Int? = -1
        val socket = WsSocket(
            "ws://127.0.0.1:${server.localPort}/relay", "Bearer x",
            {}, { code -> closed = code })
        assertFalse(socket.connect())
        assertNull(closed)
        thread.join(5000)
    }

    @Test
    fun missingAcceptRefused() {
        val server = ServerSocket(0)
        val thread = Thread {
            val peer = server.accept()
            readRequest(peer.getInputStream())
            peer.getOutputStream().write(
                ("HTTP/1.1 101 Switching Protocols\r\n" +
                    "Upgrade: websocket\r\n" +
                    "Connection: Upgrade\r\n" +
                    "\r\n").encodeToByteArray())
            peer.getOutputStream().flush()
            peer.getInputStream().read()
            peer.close()
            server.close()
        }
        thread.start()
        var closed: Int? = -1
        val socket = WsSocket(
            "ws://127.0.0.1:${server.localPort}/relay", "Bearer x",
            {}, { code -> closed = code })
        assertFalse(socket.connect())
        assertNull(closed)
        thread.join(5000)
    }

    /** A keystore made by keytool at test time, since the JDK signs with it. */
    private fun selfSigned(): KeyStore {
        val keystore = KeyStore.getInstance("PKCS12")
        val file = java.io.File.createTempFile("ws-test", ".p12")
        file.deleteOnExit()
        // keytool refuses to fill a keystore file that exists but is empty.
        assertTrue(file.delete())
        // java.home inside the test JVM may be a bare runtime; JAVA_HOME holds the full JDK.
        val jdkHome = System.getenv("JAVA_HOME")?.takeIf {
            java.io.File(it, "bin/keytool").exists()
        } ?: System.getProperty("java.home")
        val keytool = ProcessBuilder(
            "$jdkHome/bin/keytool",
            "-genkeypair", "-alias", "ws", "-dname", "CN=localhost",
            "-keyalg", "RSA", "-keysize", "2048", "-validity", "1",
            "-storetype", "PKCS12", "-keystore", file.absolutePath, "-storepass", "password")
        val process = keytool.start()
        val stderr = process.errorStream.readBytes().decodeToString()
        val done = process.waitFor()
        assertEquals("keytool failed: $stderr", 0, done)
        file.inputStream().use { keystore.load(it, "password".toCharArray()) }
        return keystore
    }

    /** A client factory trusting anything, so the hostname verifier is the only check. */
    private fun trustingFactory(): SSLSocketFactory {
        val context = SSLContext.getInstance("TLS")
        context.init(null, arrayOf(object : X509TrustManager {
            override fun checkClientTrusted(chain: Array<X509Certificate>, authType: String) {}
            override fun checkServerTrusted(chain: Array<X509Certificate>, authType: String) {}
            override fun getAcceptedIssuers(): Array<X509Certificate> = arrayOf()
        }), SecureRandom())
        return context.socketFactory
    }

    @Test
    fun tlsHostnameMatchConnects() {
        val keys = selfSigned()
        val context = SSLContext.getInstance("TLS")
        context.init(
            KeyManagerFactory.getInstance(KeyManagerFactory.getDefaultAlgorithm()).apply {
                init(keys, "password".toCharArray())
            }.keyManagers,
            null, null)
        val server = context.serverSocketFactory.createServerSocket(0)
        val texts = ArrayList<String>()
        val thread = Thread {
            val peer = server.accept()
            val input = peer.getInputStream()
            var key = ""
            while (true) {
                val l = line(input)
                if (l.isEmpty()) break
                if (l.startsWith("Sec-WebSocket-Key:", ignoreCase = true)) {
                    key = l.substringAfter(":").trim()
                }
            }
            writeHandshake(peer.getOutputStream(), acceptFor(key))
            peer.getOutputStream().write(serverFrame(WsFrame.TEXT, "ok".encodeToByteArray()))
            peer.getOutputStream().flush()
            peer.getInputStream().read()
            peer.close()
            server.close()
        }
        thread.start()
        var closed: Int? = -1
        val socket = WsSocket(
            "wss://localhost:${server.localPort}/relay", "Bearer x",
            { bytes -> texts.add(bytes.decodeToString()) }, { code -> closed = code },
            HostnameVerifier { host, _ -> host == "localhost" }, trustingFactory())
        assertTrue(socket.connect())
        val deadline = System.currentTimeMillis() + 5000
        while (texts.isEmpty() && System.currentTimeMillis() < deadline) Thread.sleep(10)
        socket.close()
        thread.join(5000)
        assertEquals(listOf("ok"), texts)
    }

    @Test
    fun tlsHostnameMismatchRefusedBeforeRequestBytes() {
        val keys = selfSigned()
        val context = SSLContext.getInstance("TLS")
        context.init(
            KeyManagerFactory.getInstance(KeyManagerFactory.getDefaultAlgorithm()).apply {
                init(keys, "password".toCharArray())
            }.keyManagers,
            null, null)
        val server = context.serverSocketFactory.createServerSocket(0)
        var serverSawBytes = false
        val thread = Thread {
            val peer = server.accept() as SSLSocket
            peer.startHandshake()
            // The client must close before writing anything; a read blocks then sees EOF.
            serverSawBytes = peer.getInputStream().read() != -1
            peer.close()
            server.close()
        }
        thread.start()
        var closed: Int? = -1
        val socket = WsSocket(
            "wss://localhost:${server.localPort}/relay", "Bearer secret-token",
            {}, { code -> closed = code },
            HostnameVerifier { _, _ -> false }, trustingFactory())
        assertFalse(socket.connect())
        assertNull(closed)
        socket.close()
        thread.join(5000)
        // The server read EOF, not the Authorization header.
        assertFalse(serverSawBytes)
    }
}