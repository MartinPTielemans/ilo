package app.ilo.browser.android.sync

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ScryptTest {

    @Test
    fun rfc7914Vector1() {
        // The RFC's first test vector, the one ilo's parameters are a scaled version of.
        val key = Crypto.scrypt(
            password = "".encodeToByteArray(),
            salt = "".encodeToByteArray(),
            n = 16, r = 1, p = 1, length = 64)
        assertArrayEquals(
            hex(
                "77d6576238657b203b19ca42c18a0497f16b4844e3074ae8dfdffa3fede21442" +
                    "fcd0069ded0948f8326a753a0fc81f17e8d3e0fb2e0d3628cf35e20c38d18906"),
            key)
    }

    @Test
    fun rfc7914Vector2() {
        val key = Crypto.scrypt(
            password = "password".encodeToByteArray(),
            salt = "NaCl".encodeToByteArray(),
            n = 1024, r = 8, p = 16, length = 64)
        assertArrayEquals(
            hex(
                "fdbabe1c9d3472007856e7190d01e9fe7c6ad7cbc8237830e77376634b373162" +
                    "2eaf30d92e22a3886ff109279d9830dac727afb94a83ee6d8360cbdfa2cc0640"),
            key)
    }

    @Test
    fun pbkdf2Vector() {
        // One iteration of PBKDF2-HMAC-SHA-256, the fixed start of every scrypt call.
        val key = Crypto.pbkdf2One(
            password = "passwd".encodeToByteArray(),
            salt = "salt".encodeToByteArray(),
            length = 64)
        assertArrayEquals(
            hex(
                "55ac046e56e3089fec1691c22544b605f94185216dde0465e68b9d57c20dacbc" +
                    "49ca9cccf179b645991664b39d77ef317c71b845b1e30bd509112041d3a19783"),
            key)
    }

    @Test
    fun chachaRoundTrip() {
        val key = ByteArray(32) { it.toByte() }
        val nonce = ByteArray(12) { (it + 1).toByte() }
        val plaintext = "the browser's keybag".encodeToByteArray()
        val sealed = Crypto.chaChaSeal(key, nonce, plaintext, ByteArray(0))!!
        val opened = Crypto.chaChaOpen(key, nonce, sealed.first, sealed.second, ByteArray(0))
        assertArrayEquals(plaintext, opened)
    }

    @Test
    fun chachaWrongKey() {
        val key = ByteArray(32) { it.toByte() }
        val nonce = ByteArray(12)
        val plaintext = "sealed".encodeToByteArray()
        val sealed = Crypto.chaChaSeal(key, nonce, plaintext, ByteArray(0))!!
        val other = Crypto.chaChaOpen(ByteArray(32) { (it + 9).toByte() }, nonce,
            sealed.first, sealed.second, ByteArray(0))
        assertNull(other)
    }

    @Test
    fun x25519Rfc7748Vector() {
        // The RFC's Diffie-Hellman vector: two fixed scalars agree on the shared secret.
        val a = "77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a"
        val b = "5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb"
        val expected = "4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742"
        val alicePublic = Crypto.x25519PublicKey(hex(a))
        assertArrayEquals(
            hex("8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a"),
            alicePublic)
        assertArrayEquals(
            hex("de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f"),
            Crypto.x25519PublicKey(hex(b)))
        // Each side's private key with the other's public gives the same shared secret.
        assertArrayEquals(hex(expected), Crypto.x25519Shared(hex(a), Crypto.x25519PublicKey(hex(b))))
        assertArrayEquals(hex(expected), Crypto.x25519Shared(hex(b), alicePublic))
    }

    @Test
    fun hkdfLength() {
        val key = Crypto.hkdf(
            ikm = ByteArray(32) { 1 },
            salt = ByteArray(32) { 2 },
            info = "info".encodeToByteArray(),
            length = 32)
        assertEquals(32, key.size)
        val other = Crypto.hkdf(
            ikm = ByteArray(32) { 3 },
            salt = ByteArray(32) { 2 },
            info = "info".encodeToByteArray(),
            length = 32)
        // Different input means different output, or the derivation would be broken.
        assertEquals(false, key.contentEquals(other))
    }

    private fun hex(text: String): ByteArray =
        ByteArray(text.length / 2) { text.substring(it * 2, it * 2 + 2).toInt(16).toByte() }
}