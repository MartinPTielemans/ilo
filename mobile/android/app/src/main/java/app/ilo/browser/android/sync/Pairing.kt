// A pairing's cryptography, the side that asks for the account's secret (core/sync_pairing.cc,
// "Pairing" in the server's protocol.md). The relay carries it and learns nothing from it: the
// code both devices show is made from both keys, so a relay that swapped one would change it.

package app.ilo.browser.android.sync

class PairingRequest private constructor(private val privateKey: ByteArray) {

    constructor() : this(Crypto.randomBytes(32))

    init {
        require(privateKey.size == 32)
    }

    val publicKey: ByteArray = Crypto.x25519PublicKey(privateKey)

    /** What the request carries in place of the key, which is told only once an approver's is in. */
    val commitment: ByteArray
        get() = Crypto.sha256("ilo pairing v1 commitment".encodeToByteArray() + publicKey)

    /** The six digits both devices show. */
    fun code(approver: ByteArray): String = code(requester = publicKey, approver = approver)

    companion object {
        fun code(requester: ByteArray, approver: ByteArray): String {
            val hash = Crypto.sha256("ilo pairing v1 code".encodeToByteArray() + requester + approver)
            var number = 0L
            for (i in 0 until 8) number = number shl 8 or (hash[i].toLong() and 0xff)
            return "%06d".format(number % 1_000_000)
        }

        /** The key the secret is sealed with, from both devices' keys. */
        fun sealKey(shared: ByteArray, requester: ByteArray, approver: ByteArray): ByteArray {
            val salt = Crypto.sha256("ilo pairing v1 keys".encodeToByteArray() + requester + approver)
            return Crypto.hkdf(shared, salt, "ilo pairing v1 seal".encodeToByteArray(), 32)
        }
    }

    /** The account's secret out of what the approver sealed for this pairing. */
    fun open(sealed: ByteArray, approver: ByteArray, pairing: String): ByteArray? {
        if (sealed.size != RecoveryWords.SECRET_LENGTH + 16) return null
        val shared = Crypto.x25519Shared(privateKey, approver) ?: return null
        // CryptoKit rejects a low-order key by its all-zero secret; x25519Shared does the same.
        val key = sealKey(shared, publicKey, approver)
        val nonce = ByteArray(12)
        return Crypto.chaChaOpen(
            key, nonce,
            sealed.copyOfRange(0, RecoveryWords.SECRET_LENGTH),
            sealed.copyOfRange(RecoveryWords.SECRET_LENGTH, sealed.size),
            pairing.encodeToByteArray())
    }
}