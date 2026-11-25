package app.ilo.browser.android.sync

import java.util.concurrent.ConcurrentLinkedQueue
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Disconnect against the real account object: it keeps the account's Spaces, drops the sign-in,
 * and a later sign-in puts the phone back among the account's devices. */
class DisconnectTest {

    /** A state with one Space the phone already downloaded, its sign-in planted as the app's
     * own files would have it. */
    private fun plantedAccount(wire: Wire? = null): Triple<SyncedAccount, MemorySecretStore, MemoryStateStore> {
        val secrets = MemorySecretStore()
        val store = MemoryStateStore()
        val session = Session(
            "https://sync.example", "access", Long.MAX_VALUE, "refresh",
            "account", "device", "a@example", "guid")
        secrets.write("session", session.serialized().encodeToByteArray())
        store.mark()
        val entity = SpaceEntity()
        entity.id = "space-1"
        entity.name = "Work"
        val state = SyncState()
        state.downloaded = true
        state.records[entity.id!!] = Record(
            "server-1", 5, 1000, "hash-1", entity, false)
        store.save(state.serialized().encodeToByteArray())
        val account = SyncedAccount(secrets, store, wire = wire)
        return Triple(account, secrets, store)
    }

    @Test
    fun `keeps the spaces and drops the sign-in`() = runTest {
        val (account, secrets, store) = plantedAccount()

        assertTrue(account.spaces.value.isNotEmpty())
        account.disconnect()

        assertEquals(AccountState.SignedOut, account.state.value)
        assertEquals("", account.email.value)
        // The Spaces stay for the UI to keep showing, and the state stays for the next
        // start, which a disconnect brings back with no sign-in and nothing to sync.
        assertTrue(account.spaces.value.isNotEmpty())
        assertNotNull(store.load())
        assertEquals(null, secrets.read("session"))
        assertEquals(null, secrets.read("secret"))
        assertEquals(null, secrets.read("keys"))
    }

    @Test
    fun `disconnect is idempotent`() = runTest {
        val (account, _, _) = plantedAccount()

        account.disconnect()
        account.disconnect()

        assertEquals(AccountState.SignedOut, account.state.value)
        assertTrue(account.spaces.value.isNotEmpty())
    }

    @Test
    fun `signing out erases what disconnect keeps`() = runTest {
        val (account, _, store) = plantedAccount()

        account.signOut()

        assertEquals(AccountState.SignedOut, account.state.value)
        assertEquals(null, store.load())
        assertTrue(account.spaces.value.isEmpty())
    }

    /** The wire as a test wants it: it counts the sync commands and holds the first one inside
     * the call until [hold] is let go, then answers nothing. The hold is a spin, not a
     * suspension, so cancelling the cycle can't cut it short: the failure lands only when the
     * test says, after the account was signed out under it. */
    private class HeldWire {
        val paths = ConcurrentLinkedQueue<String>()
        val commands = AtomicInteger()
        val released = AtomicInteger()
        @Volatile var hold = false

        fun asWire(): Wire = { request, _ ->
            val path = request.url.substringAfter("sync.example")
            paths.add(path)
            if (path.startsWith("/command/")) {
                commands.incrementAndGet()
                val start = System.nanoTime()
                while (hold && System.nanoTime() - start < 10_000_000_000L) { }
                released.incrementAndGet()
                throw ServerFailure.Unreachable("no answer")
            }
            Answer(ByteArray(0), 200)
        }
    }

    @Test
    fun `signing out revokes the refresh token on the wire`() = runTest {
        val wire = HeldWire()
        wire.hold = true
        val (account, secrets, _) = plantedAccount(wire.asWire())

        withTimeout(5000) { while (wire.commands.get() < 1) delay(10) }
        account.signOut()
        wire.hold = false
        withTimeout(5000) { while (wire.released.get() < 1) delay(10) }

        assertTrue(wire.paths.contains("/revoke"))
        assertNull(secrets.read("session"))
        // The dead cycle's failure lands after the sign-out and must not become a problem.
        assertEquals(null, account.problem.value)
    }

    @Test
    fun `a disconnect mid cycle leaves no retry of the dead sign-in`() = runTest {
        val wire = HeldWire()
        wire.hold = true
        val (account, secrets, _) = plantedAccount(wire.asWire())

        withTimeout(5000) { while (wire.commands.get() < 1) delay(10) }
        account.disconnect()
        wire.hold = false
        withTimeout(5000) { while (wire.released.get() < 1) delay(10) }
        // Real time, past the retry delay the failure would have set: the account's own scope
        // waits in real time, so no virtual clock can stand in for the wait.
        withContext(Dispatchers.Default) { delay(2500) }

        assertEquals(1, wire.commands.get())
        assertTrue(wire.paths.contains("/revoke"))
        assertNull(secrets.read("session"))
        assertEquals(null, account.problem.value)
    }

    @Test
    fun `a restart after disconnect restores the spaces without erasing`() = runTest {
        val (first, secrets, store) = plantedAccount()

        first.disconnect()
        val second = SyncedAccount(secrets, store)

        assertEquals(AccountState.SignedOut, second.state.value)
        assertEquals("", second.email.value)
        assertTrue(second.spaces.value.isNotEmpty())
        assertNotNull(store.load())
        assertNull(secrets.read("session"))
    }
}