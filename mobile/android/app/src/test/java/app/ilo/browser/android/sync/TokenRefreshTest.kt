package app.ilo.browser.android.sync

import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.delay
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The token refresh runs on its own coroutine, so a failure there once took the process down:
 * a phone offline at start died before it drew a thing. The refresh records its word instead,
 * and only the server's own refusal signs out. */
class TokenRefreshTest {

    /** A wire that answers nothing to a token refresh, as a phone offline reads every server. */
    private class DeadWire {
        val refreshes = AtomicInteger()

        fun asWire(): Wire = { request, _ ->
            if (request.url.substringAfter("sync.example").startsWith("/token")) {
                refreshes.incrementAndGet()
                throw ServerFailure.Unreachable("no answer")
            }
            Answer(ByteArray(0), 200)
        }
    }

    @Test
    fun `an unreachable server at start does not crash`() = runTest {
        val wire = DeadWire()
        val secrets = MemorySecretStore()
        val store = MemoryStateStore()
        // The access token already spent, so the account's first word asks for a refresh.
        val session = Session(
            "https://sync.example", "spent", 0, "refresh",
            "account", "device", "a@example", "guid")
        secrets.write("session", session.serialized().encodeToByteArray())
        store.mark()
        val account = SyncedAccount(secrets, store, wire = wire.asWire())

        // What a cold start does: the app comes to the front and the relay asks for a token.
        account.setActive(true)

        withTimeout(5000) { while (wire.refreshes.get() < 1) delay(10) }
        // The refresh failed on its own and the account still stands, waiting on its words as
        // it was, the crash's absence the point.
        assertEquals(AccountState.NeedsSecret, account.state.value)
        assertEquals(SyncedAccount.UNREACHABLE, account.problem.value)
    }

    @Test
    fun `a refused refresh token signs the device out`() = runTest {
        val secrets = MemorySecretStore()
        val store = MemoryStateStore()
        val session = Session(
            "https://sync.example", "spent", 0, "refresh",
            "account", "device", "a@example", "guid")
        secrets.write("session", session.serialized().encodeToByteArray())
        store.mark()
        val refusedWire: Wire = { _, _ -> throw ServerFailure.Refused() }
        val account = SyncedAccount(secrets, store, wire = refusedWire)

        account.setActive(true)

        withTimeout(5000) {
            while (account.state.value != AccountState.SignedOut) delay(10)
        }
        assertEquals(SyncedAccount.REMOVED, account.problem.value)
        assertNull(secrets.read("session"))
    }

    @Test
    fun `the last good token keeps serving until the server comes back`() = runTest {
        val wire = DeadWire()
        val secrets = MemorySecretStore()
        val store = MemoryStateStore()
        val session = Session(
            "https://sync.example", "access", Long.MAX_VALUE, "refresh",
            "account", "device", "a@example", "guid")
        secrets.write("session", session.serialized().encodeToByteArray())
        store.mark()
        val account = SyncedAccount(secrets, store, wire = wire.asWire())

        account.setActive(true)

        // A live token never asks: the refresh happens only once the token is spent.
        assertEquals(0, wire.refreshes.get())
        assertTrue(account.email.value.isNotEmpty())
    }
}