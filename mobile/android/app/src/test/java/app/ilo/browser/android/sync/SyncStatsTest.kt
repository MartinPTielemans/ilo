package app.ilo.browser.android.sync

import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.delay
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The account screen's word of the sync: what the last finished cycle moved, and when. */
class SyncStatsTest {

    /** A wire that hands out a set-up nigori and one Space, and takes whatever the phone sends. */
    private class StatsWire(words: String) {
        val commands = AtomicInteger()
        @Volatile var broken = false
        @Volatile private var servedOnce = false
        private val salt = ByteArray(16) { (it * 7).toByte() }
        private val key = NigoriKey.derive(words, salt)
        private val spaceId = "11111111-2222-3333-4444-555555555555"

        /** The account's keys sealed under the words' passphrase, as a computer wrote them. */
        private val specifics = run {
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

        /** The Space as GetUpdates gives it: sealed with the same key, named by its tag's hash. */
        private val spaceUpdate = run {
            val space = SpaceEntity()
            space.id = spaceId
            space.name = "Work"
            val sealed = EncryptedData(key.name, key.encrypt(CommitEntry.plaintext(space)))
            val writer = ProtoWriter()
            writer.string(1, "server-space-1")
            writer.varint(4, 1uL)
            writer.varint(6, 1uL)
            writer.message(21) {
                message(1) { sealed.write(this) }
                bytes(DataType.ILO_SPACE, ByteArray(0))
            }
            writer.string(23, clientTagHash(tag = spaceId))
            writer.toByteArray()
        }

        fun asWire(): Wire = { request, _ ->
            if (!request.url.substringAfter("sync.example").startsWith("/command/")) {
                Answer(ByteArray(0), 200)
            } else {
                if (broken) throw ServerFailure.Unreachable("gone")
                commands.incrementAndGet()
                val response = ProtoWriter()
                response.varint(4, 0uL)
                response.string(6, "b1")
                val kind = protoFields(request.body!!)?.last(3)?.varint
                when (kind) {
                    1uL -> response.message(1) {
                        // A commit: the server takes every entry it was sent.
                        val params = protoFields(request.body!!)!!.last(4)!!.bytes!!
                        val entries = protoFields(params)!!.count { it.number == 1 }
                        for (i in 0 until entries) {
                            message(1) {
                                varint(2, 1uL)
                                string(3, "server-$i")
                                varint(6, 2uL)
                            }
                        }
                    }
                    2uL -> if (servedOnce) {
                        // The nigori the words open, and the Space under the keys they hold.
                        response.message(2) {
                            message(1) {
                                string(1, "nigori-1")
                                varint(4, 1uL)
                                message(21) { bytes(DataType.NIGORI, specifics) }
                            }
                            bytes(1, spaceUpdate)
                            varint(4, 0uL)
                        }
                    } else {
                        servedOnce = true
                    }
                    else -> {}
                }
                Answer(response.toByteArray(), 200)
            }
        }
    }

    /** An account signed in, whose words the wire's nigori answers to. */
    private suspend fun settledIn(words: String, wire: StatsWire): SyncedAccount {
        val secrets = MemorySecretStore()
        val store = MemoryStateStore()
        val session = Session(
            "https://sync.example", "access", Long.MAX_VALUE, "refresh",
            "account", "device", "a@example", "guid")
        secrets.write("session", session.serialized().encodeToByteArray())
        store.mark()
        val account = SyncedAccount(secrets, store, wire = wire.asWire())
        withTimeout(5000) { while (wire.commands.get() < 1) delay(10) }
        assertTrue(account.enterRecoveryWords(words).isSuccess)
        return account
    }

    @Test
    fun `a finished cycle says what it moved`() = runTest {
        val secret = ByteArray(RecoveryWords.SECRET_LENGTH) { (it * 13 + 5).toByte() }
        val words = RecoveryWords.words(secret)
        val wire = StatsWire(words)
        val account = settledIn(words, wire)

        assertEquals(AccountState.SignedIn, account.state.value)
        assertTrue(account.spaces.value.isNotEmpty())
        val stats = account.stats.value
        assertNotNull(stats)
        // The nigori and the Space came down in the last cycle, and nothing went up.
        assertEquals(2, stats!!.downloaded)
        assertEquals(0, stats.uploaded)
        assertFalse(account.syncing.value)
    }

    @Test
    fun `the stats say the sent too`() = runTest {
        val secret = ByteArray(RecoveryWords.SECRET_LENGTH) { (it * 13 + 5).toByte() }
        val words = RecoveryWords.words(secret)
        val wire = StatsWire(words)
        val account = settledIn(words, wire)

        // A Today tab: the cycle that takes it says one up.
        val space = account.spaces.value.first().id
        assertNotNull(account.addTodayTab(space, "https://example.org/", "Example"))
        account.refresh()

        val stats = account.stats.value
        assertNotNull(stats)
        assertEquals(1, stats!!.uploaded)
        assertEquals(2, stats.downloaded)
        assertFalse(account.syncing.value)
    }

    @Test
    fun `a failed cycle leaves the last good word standing`() = runTest {
        val secret = ByteArray(RecoveryWords.SECRET_LENGTH) { (it * 13 + 5).toByte() }
        val words = RecoveryWords.words(secret)
        val wire = StatsWire(words)
        val account = settledIn(words, wire)
        val first = account.stats.value
        assertNotNull(first)

        // The server goes away: the cycle fails, and what the screen said stays.
        wire.broken = true
        account.refresh()

        assertEquals(first, account.stats.value)
        assertFalse(account.syncing.value)
    }
}