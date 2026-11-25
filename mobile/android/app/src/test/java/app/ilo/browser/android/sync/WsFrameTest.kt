package app.ilo.browser.android.sync

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test
import javax.net.ssl.HttpsURLConnection

class WsFrameTest {

    @Test
    fun hostnameVerifierDefaultsToTheSystemOne() {
        val a = WsSocket("wss://sync.ilobrowser.com/relay", "Bearer x", {}, {})
        assertSame(HttpsURLConnection.getDefaultHostnameVerifier(), a.hostnameVerifier)
        // An injected verifier reaches the socket unchanged, so a test can make it fail.
        val fake = javax.net.ssl.HostnameVerifier { _, _ -> false }
        val b = WsSocket("wss://sync.ilobrowser.com/relay", "Bearer x", {}, {}, fake)
        assertSame(fake, b.hostnameVerifier)
    }

    @Test
    fun plaintextPathHasNoVerifierSay() {
        // ws has no certificate to check, so the verifier is never consulted, and no
        // exception may leave connect(): a closed port reports onClosed(null) and false.
        var closed: Int? = -1
        val socket = WsSocket(
            "ws://127.0.0.1:1/relay", "Bearer x", {}, { code -> closed = code },
            javax.net.ssl.HostnameVerifier { _, _ -> throw AssertionError("verifier asked") })
        assertFalse(socket.connect())
        assertNull(closed)
    }

    @Test
    fun clientFramesAreMasked() {
        val frame = WsFrame(WsFrame.TEXT, "hello".encodeToByteArray())
        val bytes = frame.serialized()
        // FIN + text opcode, length 5, mask bit set, four mask bytes, five masked bytes.
        assertEquals(0x81, bytes[0].toInt() and 0xff)
        assertEquals(5, bytes[1].toInt() and 0x7f)
        assertEquals(0x80, bytes[1].toInt() and 0x80)
        assertEquals(11, bytes.size)
    }

    @Test
    fun parseServerTextFrame() {
        val bytes = byteArrayOf(0x81.toByte(), 0x05) + "hello".encodeToByteArray()
        val frames = ArrayList<WsFrame>()
        assertTrue(WsFrame.parse(bytes, frames))
        assertEquals(1, frames.size)
        assertEquals(WsFrame.TEXT, frames[0].opcode)
        assertEquals("hello", frames[0].payload.decodeToString())
    }

    @Test
    fun parseExtendedLength() {
        val payload = ByteArray(200) { it.toByte() }
        val bytes = byteArrayOf(0x81.toByte(), 126.toByte(), 0.toByte(), 200.toByte()) + payload
        val frames = ArrayList<WsFrame>()
        assertTrue(WsFrame.parse(bytes, frames))
        assertArrayEquals(payload, frames[0].payload)
    }

    @Test
    fun lengthBoundaries() {
        // 125 stays in the short form, 126 and 65535 go in the 16-bit one.
        for (size in listOf(125, 126, 65535)) {
            val payload = ByteArray(size) { (it % 251).toByte() }
            val frames = ArrayList<WsFrame>()
            val header = when {
                size < 126 -> byteArrayOf(0x81.toByte(), size.toByte())
                else -> byteArrayOf(
                    0x81.toByte(), 126.toByte(), (size shr 8).toByte(), size.toByte())
            }
            assertTrue("size $size", WsFrame.parse(header + payload, frames))
            assertEquals("size $size", 1, frames.size)
            assertArrayEquals("size $size", payload, frames[0].payload)
        }
    }

    @Test
    fun oversized64BitLengthRefused() {
        // A 64-bit length past the frame cap is a broken stream, not a slow one.
        val bytes = byteArrayOf(
            0x81.toByte(), 127.toByte(),
            0x7f.toByte(), 0xff.toByte(), 0xff.toByte(), 0xff.toByte(),
            0x00.toByte(), 0x00.toByte(), 0x00.toByte(), 0x00.toByte())
        val frames = ArrayList<WsFrame>()
        assertFalse(WsFrame.parse(bytes, frames))
    }

    @Test
    fun parseTwoFramesAtOnce() {
        val one = byteArrayOf(0x81.toByte(), 0x01) + "a".encodeToByteArray()
        val two = byteArrayOf(0x81.toByte(), 0x01) + "b".encodeToByteArray()
        val frames = ArrayList<WsFrame>()
        assertTrue(WsFrame.parse(one + two, frames))
        assertEquals(2, frames.size)
        assertEquals("a", frames[0].payload.decodeToString())
        assertEquals("b", frames[1].payload.decodeToString())
    }

    @Test
    fun partialFrameWaits() {
        val whole = byteArrayOf(0x81.toByte(), 0x03) + "abc".encodeToByteArray()
        val frames = ArrayList<WsFrame>()
        // Half a frame parses as nothing yet, and says so by succeeding with none.
        assertTrue(WsFrame.parse(whole.copyOf(2), frames))
        assertEquals(0, frames.size)
        assertTrue(WsFrame.parse(whole, frames))
        assertEquals(1, frames.size)
    }

    @Test
    fun maskedServerFrameRefused() {
        // A server frame with the mask bit set isn't from a server, so the stream is bad.
        val bytes = byteArrayOf(0x81.toByte(), 0x81.toByte(), 0, 0, 0, 0, 0)
        val frames = ArrayList<WsFrame>()
        assertFalse(WsFrame.parse(bytes, frames))
    }

    @Test
    fun fragmentedFrameRefused() {
        // The relay never fragments, so a frame without FIN is a broken stream.
        val bytes = byteArrayOf(0x01, 0x01) + "a".encodeToByteArray()
        val frames = ArrayList<WsFrame>()
        assertFalse(WsFrame.parse(bytes, frames))
    }

    @Test
    fun closeCodeRead() {
        // A close frame carries its code big-endian in the first two payload bytes.
        val payload = byteArrayOf(0x0f.toByte(), 0xa1.toByte())
        val bytes = byteArrayOf(0x88.toByte(), 0x02) + payload
        val frames = ArrayList<WsFrame>()
        assertTrue(WsFrame.parse(bytes, frames))
        assertEquals(WsFrame.CLOSE, frames[0].opcode)
        assertEquals(4001, ((frames[0].payload[0].toInt() and 0xff) shl 8) or
            (frames[0].payload[1].toInt() and 0xff))
    }
}