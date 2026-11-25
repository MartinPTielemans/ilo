package app.ilo.browser.android.sync

import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.delay
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** Entering the recovery words against the account: the words carry their own refresh, so a
 * nigori the phone read before the computer set the account up is asked for again. */
class EnterWordsTest {

    /** The account's keys sealed under the words' passphrase, as a computer wrote them. */
    private class SetUpWire(words: String) {
        val commands = AtomicInteger()
        @Volatile var setUp = false
        private val salt = ByteArray(16) { (it * 7).toByte() }
        private val specifics = run {
            val key = NigoriKey.derive(words, salt)
            val plain = ProtoWriter()
            plain.message(2) {
                bytes(3, key.encryption)
                bytes(4, key.mac)
            }
            val sealed = EncryptedData(key.name, key.encrypt(plain.toByteArray()))
            val writer = ProtoWriter()
            writer.message(1) { sealed.write(this) }
            writer.varint(30, NigoriSpecifics.CUSTOM_PASSPHRASE.toULong())
            writer.varint(45, NigoriSpecifics.SCRYPT.toULong())
            writer.string(46, salt.base64())
            writer.toByteArray()
        }

        fun asWire(): Wire = { request, _ ->
            if (!request.url.substringAfter("sync.example").startsWith("/command/")) {
                Answer(ByteArray(0), 200)
            } else {
                commands.incrementAndGet()
                val response = ProtoWriter()
                response.varint(4, 0uL)
                response.string(6, "b1")
                if (setUp) response.message(2) {
                    message(1) {
                        string(1, "nigori-1")
                        varint(4, 1uL)
                        message(21) { bytes(DataType.NIGORI, specifics) }
                    }
                    varint(4, 0uL)
                }
                Answer(response.toByteArray(), 200)
            }
        }
    }

    /** An account signed in whose nigori is the placeholder a first download left behind: the
     * computer hadn't set the account up when the phone last asked. */
    private fun planted(wire: Wire): SyncedAccount {
        val secrets = MemorySecretStore()
        val store = MemoryStateStore()
        val session = Session(
            "https://sync.example", "access", Long.MAX_VALUE, "refresh",
            "account", "device", "a@example", "guid")
        secrets.write("session", session.serialized().encodeToByteArray())
        store.mark()
        val state = SyncState()
        state.nigori = NigoriSpecifics()
        store.save(state.serialized().encodeToByteArray())
        return SyncedAccount(secrets, store, wire = wire)
    }

    @Test
    fun `words refresh a placeholder nigori and take the account`() = runTest {
        val secret = ByteArray(RecoveryWords.SECRET_LENGTH) { (it * 13 + 5).toByte() }
        val words = RecoveryWords.words(secret)
        val wire = SetUpWire(words)
        val account = planted(wire.asWire())

        withTimeout(5000) { while (wire.commands.get() < 1) delay(10) }
        assertEquals(AccountState.NeedsSecret, account.state.value)
        // The computer sets the account up only after the phone's first download.
        wire.setUp = true

        val result = account.enterRecoveryWords(words)

        assertTrue(result.isSuccess)
        assertEquals(AccountState.SignedIn, account.state.value)
        assertEquals(null, account.problem.value)
        assertEquals(words.split(" "), account.recoveryWords())
    }

    @Test
    fun `words with nothing set up say to set up first`() = runTest {
        val secret = ByteArray(RecoveryWords.SECRET_LENGTH) { (it * 13 + 5).toByte() }
        val words = RecoveryWords.words(secret)
        val wire = SetUpWire(words)
        val account = planted(wire.asWire())

        withTimeout(5000) { while (wire.commands.get() < 1) delay(10) }

        val result = account.enterRecoveryWords(words)

        assertTrue(result.isFailure)
        assertEquals(AccountState.NeedsSecret, account.state.value)
        assertEquals(SyncedAccount.SET_UP_FIRST, account.problem.value)
    }

    @Test
    fun `words not on the list never reach the server`() = runTest {
        val secret = ByteArray(RecoveryWords.SECRET_LENGTH) { (it * 13 + 5).toByte() }
        val wire = SetUpWire(RecoveryWords.words(secret))
        val account = planted(wire.asWire())

        withTimeout(5000) { while (wire.commands.get() < 1) delay(10) }

        val result = account.enterRecoveryWords("not the account's words")

        assertTrue(result.isFailure)
        assertEquals(1, wire.commands.get())
        assertEquals(SyncedAccount.SET_UP_FIRST, account.problem.value)
    }
}