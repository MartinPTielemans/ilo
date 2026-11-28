// The crypto the sync client needs, all from javax.crypto and java.security, with X25519 and
// ChaCha20-Poly1305 from their JCA names (JDK 11+/Android API 33+; API 29 falls back below).

package app.ilo.browser.android.sync

import java.math.BigInteger
import java.security.MessageDigest
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.Mac
import javax.crypto.spec.IvParameterSpec
import javax.crypto.spec.SecretKeySpec
import kotlin.experimental.xor

object Crypto {
    val random = SecureRandom()

    fun randomBytes(count: Int): ByteArray = ByteArray(count).also { random.nextBytes(it) }

    fun sha1(bytes: ByteArray): ByteArray = MessageDigest.getInstance("SHA-1").digest(bytes)

    fun sha256(bytes: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(bytes)

    fun hmacSha256(key: ByteArray, bytes: ByteArray): ByteArray {
        val mac = Mac.getInstance("HmacSHA256")
        // HMAC pads an empty key with zeros, so a zero block is the same key; SecretKeySpec
        // refuses the empty one the RFC's vectors use.
        mac.init(SecretKeySpec(if (key.isEmpty()) ByteArray(64) else key, "HmacSHA256"))
        return mac.doFinal(bytes)
    }

    /** A constant-time comparison, so a forged tag isn't told apart faster by timing. */
    fun constantTimeEquals(a: ByteArray, b: ByteArray): Boolean {
        if (a.size != b.size) return false
        var diff = 0
        for (i in a.indices) diff = diff or (a[i].toInt() xor b[i].toInt())
        return diff == 0
    }

    /** PBKDF2-HMAC-SHA256 with the one iteration scrypt uses, for scrypt's blocks. */
    fun pbkdf2One(password: ByteArray, salt: ByteArray, length: Int): ByteArray {
        val out = ArrayList<Byte>(length)
        var number = 1
        while (out.size < length) {
            var mac = hmacSha256(
                password,
                salt + byteArrayOf(
                    (number shr 24).toByte(), (number shr 16 and 0xff).toByte(),
                    (number shr 8 and 0xff).toByte(), (number and 0xff).toByte()))
            out.addAll(mac.toList())
            number++
        }
        return ByteArray(length) { out[it] }
    }

    /** HKDF (RFC 5869) with SHA-256, as CryptoKit's hkdfDerivedSymmetricKey is. */
    fun hkdf(ikm: ByteArray, salt: ByteArray, info: ByteArray, length: Int): ByteArray {
        val prk = hmacSha256(salt, ikm)
        var t = ByteArray(0)
        val out = ArrayList<Byte>(length)
        var i = 1
        while (out.size < length) {
            t = hmacSha256(prk, t + info + byteArrayOf(i.toByte()))
            out.addAll(t.toList())
            i++
        }
        return ByteArray(length) { out[it] }
    }

    /** AES-128-CBC with PKCS#7 padding; null when the input is the wrong length for a block. */
    fun aesCbc(mode: Int, key: ByteArray, iv: ByteArray, input: ByteArray): ByteArray? {
        if (key.size != 16 || iv.size != 16) return null
        return try {
            val cipher = Cipher.getInstance("AES/CBC/PKCS5Padding")
            cipher.init(mode, SecretKeySpec(key, "AES"), IvParameterSpec(iv))
            cipher.doFinal(input)
        } catch (_: Exception) {
            null
        }
    }

    // MARK: scrypt (RFC 7914)

    fun scrypt(password: ByteArray, salt: ByteArray, n: Int, r: Int, p: Int, length: Int): ByteArray {
        require(n > 1 && n and (n - 1) == 0 && r > 0 && p > 0)
        val words = 32 * r
        var block = pbkdf2One(password, salt, p * words * 4)

        // scrypt's words are little-endian, as every phone's are.
        val ints = IntArray(block.size / 4)
        for (i in ints.indices) {
            ints[i] = (block[4 * i].toInt() and 0xff) or ((block[4 * i + 1].toInt() and 0xff) shl 8) or
                ((block[4 * i + 2].toInt() and 0xff) shl 16) or ((block[4 * i + 3].toInt() and 0xff) shl 24)
        }
        // Each lane has its own part of the block, so plain Int wrapping arithmetic is safe.
        for (lane in 0 until p) roMix(ints, lane * words, n, r)
        block = ByteArray(ints.size * 4)
        for (i in ints.indices) {
            block[4 * i] = (ints[i].toByte())
            block[4 * i + 1] = (ints[i] shr 8).toByte()
            block[4 * i + 2] = (ints[i] shr 16).toByte()
            block[4 * i + 3] = (ints[i] shr 24).toByte()
        }
        return pbkdf2One(password, block, length)
    }

    /** Salsa20/8 core over one 64-byte block, in place. */
    private fun salsa8(b: IntArray) {
        var x0 = b[0]; var x1 = b[1]; var x2 = b[2]; var x3 = b[3]
        var x4 = b[4]; var x5 = b[5]; var x6 = b[6]; var x7 = b[7]
        var x8 = b[8]; var x9 = b[9]; var x10 = b[10]; var x11 = b[11]
        var x12 = b[12]; var x13 = b[13]; var x14 = b[14]; var x15 = b[15]
        repeat(4) {
            x4 = x4 xor rotate(x0 + x12, 7); x8 = x8 xor rotate(x4 + x0, 9)
            x12 = x12 xor rotate(x8 + x4, 13); x0 = x0 xor rotate(x12 + x8, 18)
            x9 = x9 xor rotate(x5 + x1, 7); x13 = x13 xor rotate(x9 + x5, 9)
            x1 = x1 xor rotate(x13 + x9, 13); x5 = x5 xor rotate(x1 + x13, 18)
            x14 = x14 xor rotate(x10 + x6, 7); x2 = x2 xor rotate(x14 + x10, 9)
            x6 = x6 xor rotate(x2 + x14, 13); x10 = x10 xor rotate(x6 + x2, 18)
            x3 = x3 xor rotate(x15 + x11, 7); x7 = x7 xor rotate(x3 + x15, 9)
            x11 = x11 xor rotate(x7 + x3, 13); x15 = x15 xor rotate(x11 + x7, 18)
            x1 = x1 xor rotate(x0 + x3, 7); x2 = x2 xor rotate(x1 + x0, 9)
            x3 = x3 xor rotate(x2 + x1, 13); x0 = x0 xor rotate(x3 + x2, 18)
            x6 = x6 xor rotate(x5 + x4, 7); x7 = x7 xor rotate(x6 + x5, 9)
            x4 = x4 xor rotate(x7 + x6, 13); x5 = x5 xor rotate(x4 + x7, 18)
            x11 = x11 xor rotate(x10 + x9, 7); x8 = x8 xor rotate(x11 + x10, 9)
            x9 = x9 xor rotate(x8 + x11, 13); x10 = x10 xor rotate(x9 + x8, 18)
            x12 = x12 xor rotate(x15 + x14, 7); x13 = x13 xor rotate(x12 + x15, 9)
            x14 = x14 xor rotate(x13 + x12, 13); x15 = x15 xor rotate(x14 + x13, 18)
        }
        b[0] += x0; b[1] += x1; b[2] += x2; b[3] += x3
        b[4] += x4; b[5] += x5; b[6] += x6; b[7] += x7
        b[8] += x8; b[9] += x9; b[10] += x10; b[11] += x11
        b[12] += x12; b[13] += x13; b[14] += x14; b[15] += x15
    }

    private fun rotate(value: Int, count: Int): Int =
        value shl count or (value ushr (32 - count))

    private fun roMix(block: IntArray, offset: Int, n: Int, r: Int) {
        val words = 32 * r
        val table = IntArray(words * n)
        val x = IntArray(16)
        for (i in 0 until n) {
            System.arraycopy(block, offset, table, i * words, words)
            blockMix(block, offset, x, r)
        }
        repeat(n) {
            // The last 64 bytes as a little-endian number, modulo n: its first word is enough.
                        val row = (block[offset + words - 16] and (n - 1)) * words
            for (k in 0 until words) block[offset + k] = block[offset + k] xor table[row + k]
            blockMix(block, offset, x, r)
        }
    }

    private fun blockMix(block: IntArray, offset: Int, x: IntArray, r: Int) {
        System.arraycopy(block, offset + (2 * r - 1) * 16, x, 0, 16)
        // The mixed words go to a fresh array: written in place, an early Y would overwrite a B
        // that hasn't been read yet.
        val out = IntArray(32 * r)
        for (i in 0 until 2 * r) {
            val part = offset + i * 16
            for (k in 0 until 16) x[k] = x[k] xor block[part + k]
            salsa8(x)
            // Even parts first, then the odd ones.
            System.arraycopy(x, 0, out, (i / 2 + (i and 1) * r) * 16, 16)
        }
        System.arraycopy(out, 0, block, offset, 32 * r)
    }

    // MARK: X25519 (RFC 7748). BigInteger field math: the pairing runs it once with an
    // ephemeral key, so speed and constant-time don't matter here, and JCA has no X25519 on
    // API 29.

    private val curveP = BigInteger.TWO.pow(255).subtract(BigInteger.valueOf(19))

    /** The public key for a 32-byte private one, as CryptoKit's Curve25519 gives. */
    fun x25519PublicKey(privateKey: ByteArray): ByteArray {
        val k = clamp(privateKey)
        return ladder(BigInteger.valueOf(9), k)
    }

    /** X25519 key agreement. Null on an all-zero shared secret, which is what a low-order key
     * gives: as the Swift code rejects it, so does this. */
    fun x25519Shared(privateKey: ByteArray, publicKey: ByteArray): ByteArray? {
        val k = clamp(privateKey)
        // The peer's coordinate has its top bit masked off, as RFC 7748 takes it. A mask of
        // 2^255-1, not of p: p's own low bits would clear bits of the coordinate itself.
        val u = BigInteger(1, publicKey.reversedArray()).and(BigInteger.TWO.pow(255).subtract(BigInteger.ONE))
        val shared = ladder(u, k)
        return if (shared.all { it.toInt() == 0 }) null else shared
    }

    private fun clamp(privateKey: ByteArray): ByteArray {
        val k = privateKey.copyOf(32)
        k[0] = (k[0].toInt() and 248).toByte()
        k[31] = ((k[31].toInt() and 127) or 64).toByte()
        return k
    }

    /** The Montgomery ladder of RFC 7748, over BigIntegers reduced mod 2^255-19. */
    private fun ladder(u: BigInteger, scalar: ByteArray): ByteArray {
        val a24 = BigInteger.valueOf(121665)
        var x2 = BigInteger.ONE
        var z2 = BigInteger.ZERO
        var x3 = u
        var z3 = BigInteger.ONE
        var swap = 0
        for (t in 254 downTo 0) {
            val bit = (scalar[t / 8].toInt() shr (t % 8)) and 1
            val choice = bit xor swap
            if (choice == 1) {
                var tmp = x2; x2 = x3; x3 = tmp
                tmp = z2; z2 = z3; z3 = tmp
            }
            swap = bit
            val a = x2.add(z2).mod(curveP)
            val aa = a.multiply(a).mod(curveP)
            val b = x2.subtract(z2).mod(curveP)
            val bb = b.multiply(b).mod(curveP)
            val e = aa.subtract(bb).mod(curveP)
            val c = x3.add(z3).mod(curveP)
            val d = x3.subtract(z3).mod(curveP)
            val da = d.multiply(a).mod(curveP)
            val cb = c.multiply(b).mod(curveP)
            x3 = da.add(cb).mod(curveP).let { it.multiply(it).mod(curveP) }
            z3 = u.multiply(da.subtract(cb).mod(curveP).let { it.multiply(it) }).mod(curveP)
            x2 = aa.multiply(bb).mod(curveP)
            z2 = e.multiply(aa.add(a24.multiply(e)).mod(curveP)).mod(curveP)
        }
        if (swap == 1) {
            var tmp = x2; x2 = x3; x3 = tmp
            tmp = z2; z2 = z3; z3 = tmp
        }
        val result = x2.multiply(z2.modPow(curveP.subtract(BigInteger.TWO), curveP)).mod(curveP)
        val out = ByteArray(32)
        val bytes = result.toByteArray()  // big-endian, maybe with a leading zero or 33 bytes
        for (i in 0 until 32) {
            val from = bytes.size - 1 - i
            out[i] = if (from >= 0) bytes[from] else 0
        }
        return out
    }

    // MARK: ChaCha20-Poly1305 (RFC 8439), in pure Kotlin: JCA has no "ChaCha20-Poly1305" on
    // API 29, and CryptoKit's ChaChaPoly is what the iOS side seals with.

    /** ChaCha20 block (counter starts at 1 for the AEAD). */
    private fun chachaBlock(key: ByteArray, counter: Int, nonce: ByteArray): IntArray {
        val state = IntArray(16)
        val constants = intArrayOf(0x61707865, 0x3320646e, 0x79622d32, 0x6b206574)
        System.arraycopy(constants, 0, state, 0, 4)
        for (i in 0 until 8) {
            state[4 + i] = (key[4 * i].toInt() and 0xff) or ((key[4 * i + 1].toInt() and 0xff) shl 8) or
                ((key[4 * i + 2].toInt() and 0xff) shl 16) or ((key[4 * i + 3].toInt() and 0xff) shl 24)
        }
        state[12] = counter
        for (i in 0 until 3) {
            state[13 + i] = (nonce[4 * i].toInt() and 0xff) or ((nonce[4 * i + 1].toInt() and 0xff) shl 8) or
                ((nonce[4 * i + 2].toInt() and 0xff) shl 16) or ((nonce[4 * i + 3].toInt() and 0xff) shl 24)
        }
        val working = state.copyOf()
        for (round in 0 until 10) {
            quarter(working, 0, 4, 8, 12)
            quarter(working, 1, 5, 9, 13)
            quarter(working, 2, 6, 10, 14)
            quarter(working, 3, 7, 11, 15)
            quarter(working, 0, 5, 10, 15)
            quarter(working, 1, 6, 11, 12)
            quarter(working, 2, 7, 8, 13)
            quarter(working, 3, 4, 9, 14)
        }
        for (i in 0 until 16) working[i] += state[i]
        return working
    }

    private fun quarter(x: IntArray, a: Int, b: Int, c: Int, d: Int) {
        x[a] += x[b]; x[d] = x[d] xor x[a]; x[d] = x[d] shl 16 or (x[d] ushr 16)
        x[c] += x[d]; x[b] = x[b] xor x[c]; x[b] = x[b] shl 12 or (x[b] ushr 20)
        x[a] += x[b]; x[d] = x[d] xor x[a]; x[d] = x[d] shl 8 or (x[d] ushr 24)
        x[c] += x[d]; x[b] = x[b] xor x[c]; x[b] = x[b] shl 7 or (x[b] ushr 25)
    }

    /** The Poly1305 key from ChaCha20's first block, and the tag over AAD and ciphertext. */
    private fun poly1305(key: ByteArray, aad: ByteArray, ciphertext: ByteArray): ByteArray {
        // Poly1305 with BigIntegers, over 5-bit limbs would be the fast way; one-shot use
        // makes the schoolbook clamping approach below enough.
        // The r clamp of RFC 8439, on each byte: the little-endian number has its top bits
        // of bytes 3, 7, 11, 15 and the bottom two of 4, 8, 12 cleared.
        val rb = key.copyOfRange(0, 16)
        rb[3] = (rb[3].toInt() and 15).toByte()
        rb[7] = (rb[7].toInt() and 15).toByte()
        rb[11] = (rb[11].toInt() and 15).toByte()
        rb[15] = (rb[15].toInt() and 15).toByte()
        rb[4] = (rb[4].toInt() and 252).toByte()
        rb[8] = (rb[8].toInt() and 252).toByte()
        rb[12] = (rb[12].toInt() and 252).toByte()
        val r = BigInteger(1, rb.reversedArray())
        val s = BigInteger(1, key.copyOfRange(16, 32))
        val p = BigInteger.TWO.pow(130).subtract(BigInteger.valueOf(5))
        var acc = BigInteger.ZERO
        fun absorb(bytes: ByteArray, padded: Boolean) {
            var i = 0
            while (i < bytes.size) {
                val n = minOf(16, bytes.size - i)
                val block = bytes.copyOfRange(i, i + n)
                val value = BigInteger(1, block).or(BigInteger.ONE.shiftLeft(8 * n))
                acc = acc.add(value).multiply(r).mod(p)
                i += n
            }
        }
        absorb(aad, true)
        absorb(ciphertext, true)
        val lengths = ByteArray(16)
        for (i in 0 until 8) {
            lengths[7 - i] = (aad.size shr (8 * i)).toByte()
            lengths[15 - i] = (ciphertext.size shr (8 * i)).toByte()
        }
        acc = acc.add(BigInteger(1, lengths)).add(s)
        val tag = ByteArray(16)
        val out = acc.mod(BigInteger.TWO.pow(128)).toByteArray()
        for (i in 0 until 16) {
            val from = out.size - 1 - i
            tag[i] = if (from >= 0) out[from] else 0
        }
        return tag
    }

    private fun chachaEncrypt(key: ByteArray, nonce: ByteArray, plaintext: ByteArray): ByteArray {
        val out = ByteArray(plaintext.size)
        var counter = 1
        var i = 0
        while (i < plaintext.size) {
            val block = chachaBlock(key, counter, nonce)
            val n = minOf(64, plaintext.size - i)
            for (j in 0 until n) out[i + j] = (plaintext[i + j] xor block[j / 4].ushr(8 * (j % 4)).toByte())
            i += n
            counter++
        }
        return out
    }

    private fun polyKey(key: ByteArray, nonce: ByteArray): ByteArray {
        val block = chachaBlock(key, 0, nonce)
        val out = ByteArray(32)
        for (i in 0 until 8) {
            for (j in 0 until 4) {
                out[4 * i + j] = (block[i] ushr (8 * j)).toByte()
            }
        }
        return out
    }

    /** ChaCha20-Poly1305 decrypt, as CryptoKit's ChaChaPoly.open: a bad tag gives null. */
    fun chaChaOpen(key: ByteArray, nonce: ByteArray, ciphertext: ByteArray, tag: ByteArray,
        aad: ByteArray): ByteArray? {
        val expected = poly1305(polyKey(key, nonce), aad, ciphertext)
        if (!constantTimeEquals(expected, tag)) return null
        return chachaEncrypt(key, nonce, ciphertext)
    }

    /** ChaCha20-Poly1305 encrypt, the approver's side, for tests to seal with. */
    fun chaChaSeal(key: ByteArray, nonce: ByteArray, plaintext: ByteArray,
        aad: ByteArray): Pair<ByteArray, ByteArray>? {
        val ciphertext = chachaEncrypt(key, nonce, plaintext)
        val tag = poly1305(polyKey(key, nonce), aad, ciphertext)
        return ciphertext to tag
    }
}

/** Base64 for a URL, without padding, as the server's tokens and the relay's frames are. */
fun ByteArray.base64URL(): String =
    java.util.Base64.getUrlEncoder().withoutPadding().encodeToString(this)

fun base64URL(text: String): ByteArray? {
    val cleaned = text.replace("-", "+").replace("_", "/").trimEnd('=')
    return try {
        java.util.Base64.getDecoder().decode(
            cleaned + "=".repeat((4 - cleaned.length % 4) % 4))
    } catch (_: Exception) {
        null
    }
}

fun ByteArray.base64(): String = java.util.Base64.getEncoder().encodeToString(this)

/** A string's bytes in base64, for state lines that hold text. */
fun String.base64(): String = encodeToByteArray().base64()

fun base64(text: String): ByteArray? = try {
    java.util.Base64.getDecoder().decode(text)
} catch (_: Exception) {
    null
}