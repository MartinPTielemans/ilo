// The keys an account's data is encrypted with, as Chromium's sync makes and uses them
// (components/sync/model/crypto/nigori.cc, components/sync/nigori/), byte for byte with the
// iOS app's Nigori.swift.

package app.ilo.browser.android.sync

import javax.crypto.Cipher

/** sync_pb.EncryptedData. */
class EncryptedData(val keyName: String, val blob: String) {
    fun write(writer: ProtoWriter) {
        writer.string(1, keyName)
        writer.string(2, blob)
    }

    override fun equals(other: Any?): Boolean =
        other is EncryptedData && other.keyName == keyName && other.blob == blob

    override fun hashCode(): Int = keyName.hashCode() * 31 + blob.hashCode()

    companion object {
        fun parse(bytes: ByteArray?): EncryptedData? {
            if (bytes == null) return null
            val fields = protoFields(bytes) ?: return null
            val name = fields.last(1)?.string ?: return null
            val blob = fields.last(2)?.string ?: return null
            return EncryptedData(name, blob)
        }
    }
}

class NigoriKey(val encryption: ByteArray, val mac: ByteArray) {
    /** A custom passphrase's key, by the one derivation ilo's accounts use: SCRYPT_8192_8_11. */
    companion object {
        fun derive(passphrase: String, salt: ByteArray): NigoriKey {
            val key = Crypto.scrypt(
                passphrase.encodeToByteArray(), salt, 8192, 8, 11, 32)
            return NigoriKey(key.copyOfRange(0, 16), key.copyOfRange(16, 32))
        }
    }

    /** What a keybag and an entity call the key: the fixed text "nigori-key", encrypted with a
     * zero IV so that it's the same every time. The IV is left out of the name, as Chromium
     * and the iOS app write it: a fixed zero IV inside it says which key it is. */
    val name: String by lazy {
        val stream = byteArrayOf(0, 0, 0, 4, 0, 0, 0, 1, 0, 0, 0, 10) +
            "nigori-key".encodeToByteArray()
        val iv = ByteArray(16)
        val ciphertext = Crypto.aesCbc(Cipher.ENCRYPT_MODE, encryption, iv, stream) ?: ByteArray(0)
        (ciphertext + code(ciphertext)).base64()
    }

    fun encrypt(plaintext: ByteArray): String {
        val iv = Crypto.randomBytes(16)
        val ciphertext = Crypto.aesCbc(Cipher.ENCRYPT_MODE, encryption, iv, plaintext)
            ?: ByteArray(0)
        // Over the ciphertext alone: Chromium's kSyncNigoriAuthenticateIV is off, and computers
        // older than it read nothing else.
        return (iv + ciphertext + code(ciphertext)).base64()
    }

    fun decrypt(blob: String): ByteArray? {
        val data = base64(blob) ?: return null
        if (data.size < 16 * 2 + 32) return null
        val iv = data.copyOfRange(0, 16)
        val ciphertext = data.copyOfRange(16, data.size - 32)
        val mac = data.copyOfRange(data.size - 32, data.size)
        val overCiphertext = Crypto.hmacSha256(this.mac, ciphertext)
        val overBoth = Crypto.hmacSha256(this.mac, iv + ciphertext)
        // The older form MACs the IV with the ciphertext, as well as the ciphertext alone.
        if (!Crypto.constantTimeEquals(mac, overCiphertext) &&
            !Crypto.constantTimeEquals(mac, overBoth)
        ) return null
        return Crypto.aesCbc(Cipher.DECRYPT_MODE, encryption, iv, ciphertext)
    }

    private fun code(bytes: ByteArray): ByteArray = Crypto.hmacSha256(mac, bytes)

    override fun equals(other: Any?): Boolean =
        other is NigoriKey && other.encryption.contentEquals(encryption) &&
            other.mac.contentEquals(mac)

    override fun hashCode(): Int = encryption.contentHashCode() * 31 + mac.contentHashCode()
}

/** The account's keys by name, and the one new data is encrypted with. */
class Keybag(val keys: Map<String, NigoriKey>, val defaultName: String) {
    fun encrypt(plaintext: ByteArray): EncryptedData? =
        keys[defaultName]?.let { EncryptedData(defaultName, it.encrypt(plaintext)) }

    fun decrypt(data: EncryptedData): ByteArray? = keys[data.keyName]?.decrypt(data.blob)

    /** The keys alone are kept, not the words, and they travel as JSON-free base64 pairs. */
    fun serialized(): String = keys.entries.joinToString("|") { (name, key) ->
        name + ":" + key.encryption.base64() + ":" + key.mac.base64() + ":" + defaultName
    }

    companion object {
        fun deserialize(text: String): Keybag? {
            val keys = HashMap<String, NigoriKey>()
            var defaultName: String? = null
            for (entry in text.split("|")) {
                val parts = entry.split(":")
                if (parts.size != 4) return null
                val encryption = base64(parts[1]) ?: return null
                val mac = base64(parts[2]) ?: return null
                if (encryption.size != 16 || mac.size != 16) return null
                val key = NigoriKey(encryption, mac)
                // The name is computed, never read, so a tampered entry just fails to match.
                if (parts[0] != key.name) return null
                keys[parts[0]] = key
                defaultName = parts[3]
            }
            return Keybag(keys, defaultName ?: return null)
        }
    }
}

/** What the phone reads of sync_pb.NigoriSpecifics. */
class NigoriSpecifics {
    var keybag: EncryptedData? = null
    var passphraseType = 1
    var derivationMethod = 0
    /** Base64 of the salt's bytes. */
    var salt = ""

    constructor()

    constructor(bytes: ByteArray) {
        val fields = protoFields(bytes) ?: return
        keybag = EncryptedData.parse(fields.last(1)?.bytes)
        passphraseType = fields.last(30)?.varint?.toInt() ?: 1
        derivationMethod = fields.last(45)?.varint?.toInt() ?: 0
        salt = fields.last(46)?.string ?: ""
    }

    /** Whether a computer set the account up, with a passphrase the phone can derive a key from. */
    val isSetUp: Boolean
        get() = keybag != null && passphraseType == CUSTOM_PASSPHRASE &&
            derivationMethod == SCRYPT && base64(salt) != null

    /** The account's keys, if `passphrase` is the one their keybag was sealed with. */
    fun open(passphrase: String): Keybag? {
        if (!isSetUp) return null
        val keybag = keybag ?: return null
        val salt = base64(salt) ?: return null
        val key = NigoriKey.derive(passphrase, salt)
        if (key.name != keybag.keyName) return null
        val plain = key.decrypt(keybag.blob) ?: return null
        val fields = protoFields(plain) ?: return null
        // sync_pb.EncryptionKeys: each key's name is computed, never read.
        val keys = HashMap<String, NigoriKey>()
        for (field in fields) {
            if (field.number != 2) continue
            val inner = protoFields(field.bytes ?: return null) ?: return null
            val encryption = inner.last(3)?.bytes ?: return null
            val mac = inner.last(4)?.bytes ?: return null
            if (encryption.size != 16 || mac.size != 16) return null
            val each = NigoriKey(encryption, mac)
            keys[each.name] = each
        }
        if (!keys.containsKey(keybag.keyName)) return null
        return Keybag(keys, keybag.keyName)
    }

    override fun equals(other: Any?): Boolean =
        other is NigoriSpecifics && other.keybag == keybag &&
            other.passphraseType == passphraseType && other.derivationMethod == derivationMethod &&
            other.salt == salt

    override fun hashCode(): Int =
        (keybag?.hashCode() ?: 0) * 31 + passphraseType * 7 + derivationMethod + salt.hashCode()

    companion object {
        const val CUSTOM_PASSPHRASE = 4
        const val SCRYPT = 2
    }
}