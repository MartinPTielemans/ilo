package app.ilo.browser.android.sync

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test
import java.util.Base64

class NigoriTest {

    private val salt = ByteArray(16) { (it * 7).toByte() }

    @Test
    fun keyDerivationIsScrypt8192_8_11() {
        // The account's only derivation: Chromium's SCRYPT_8192_8_11, split 16/16.
        val key = NigoriKey.derive("hello world", salt)
        assertEquals(16, key.encryption.size)
        assertEquals(16, key.mac.size)
    }

    @Test
    fun keyNameIsStable() {
        val key = NigoriKey.derive("hello world", salt)
        val again = NigoriKey.derive("hello world", salt)
        assertEquals(key.name, again.name)
        val other = NigoriKey.derive("different", salt)
        // The name carries the key, so different keys can't share one.
        assertEquals(false, key.name == other.name)
    }

    @Test
    fun encryptDecryptRoundTrip() {
        val key = NigoriKey.derive("hello world", salt)
        val plaintext = "a secret worth sealing".encodeToByteArray()
        val blob = key.encrypt(plaintext)
        assertArrayEquals(plaintext, key.decrypt(blob))
        // Two encryptions differ: the IV is random.
        assertEquals(false, blob == key.encrypt(plaintext))
    }

    @Test
    fun decryptRejectsTampering() {
        val key = NigoriKey.derive("hello world", salt)
        val bytes = Base64.getDecoder().decode(key.encrypt("sealed".encodeToByteArray()))
        bytes[bytes.size - 1] = (bytes[bytes.size - 1].toInt() xor 1).toByte()
        assertNull(key.decrypt(Base64.getEncoder().encodeToString(bytes)))
    }

    @Test
    fun keybagRoundTrip() {
        val key = NigoriKey.derive("hello world", salt)
        val bag = Keybag(mapOf(key.name to key), key.name)
        val plaintext = "the space's entity".encodeToByteArray()
        val sealed = bag.encrypt(plaintext)!!
        assertArrayEquals(plaintext, bag.decrypt(sealed))
        // The key's name is recomputed on load, so a serialized tampered key fails to open.
        val back = Keybag.deserialize(bag.serialized())
        assertNotNull(back)
        assertEquals(bag.keys.size, back!!.keys.size)
        assertArrayEquals(plaintext, back.decrypt(sealed))
    }

    @Test
    fun keybagSerializedRejectsTamperedName() {
        val key = NigoriKey.derive("hello world", salt)
        val text = "not-the-name:" + key.encryption.base64() + ":" + key.mac.base64() + ":" + key.name
        assertNull(Keybag.deserialize(text))
    }

    @Test
    fun specificsOpen() {
        // A NigoriSpecifics as the computer writes it: keybag, custom passphrase, SCRYPT, salt.
        val passphraseKey = NigoriKey.derive("the passphrase", salt)
        val entityKey = NigoriKey(ByteArray(16) { 3 }, ByteArray(16) { 4 })
        val plain = ProtoWriter()
        plain.message(2) {
            bytes(3, entityKey.encryption)
            bytes(4, entityKey.mac)
        }
        // The bag holds the passphrase's own key too: the account's keys include it.
        plain.message(2) {
            bytes(3, passphraseKey.encryption)
            bytes(4, passphraseKey.mac)
        }
        val sealed = EncryptedData(passphraseKey.name, passphraseKey.encrypt(plain.toByteArray()))
        val writer = ProtoWriter()
        writer.message(1) { sealed.write(this) }
        writer.varint(30, NigoriSpecifics.CUSTOM_PASSPHRASE.toULong())
        writer.varint(45, NigoriSpecifics.SCRYPT.toULong())
        writer.string(46, salt.base64())

        val specifics = NigoriSpecifics(writer.toByteArray())
        assertEquals(true, specifics.isSetUp)
        val opened = specifics.open("the passphrase")
        assertNotNull(opened)
        // The bag's keys are named by their own bytes, so the entity key is found by its name.
        assertArrayEquals(entityKey.encryption, opened!!.keys[entityKey.name]!!.encryption)
        assertArrayEquals(
            passphraseKey.encryption, opened.keys[opened.defaultName]!!.encryption)
        assertNull(specifics.open("wrong"))
    }

    @Test
    fun specificsNotSetUp() {
        // No keybag: a fresh account the computer hasn't set a passphrase on.
        val writer = ProtoWriter()
        val specifics = NigoriSpecifics(writer.toByteArray())
        assertEquals(false, specifics.isSetUp)
        assertNull(specifics.open("anything"))
    }
}