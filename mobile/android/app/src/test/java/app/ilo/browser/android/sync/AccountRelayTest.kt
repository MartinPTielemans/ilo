package app.ilo.browser.android.sync

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class AccountRelayTest {

    /** A socket that records what the relay sends and lets the test speak as the server. */
    private class FakeSocket(
        val sent: MutableList<String>,
        val onText: (ByteArray) -> Unit,
    ) : AccountRelay.RelaySocket {
        var open = false

        override fun connect(): Boolean {
            open = true
            return true
        }

        override fun sendText(text: String): Boolean {
            sent.add(text)
            return open
        }

        override fun close() {
            open = false
        }
    }

    /** The account as the relay needs it, with the test's own answers. */
    private class FakeOwner : RelayOwner {
        var setUps = 0
        var syncs = 0
        var tokens = 0
        var problems = mutableListOf<String>()
        var secrets = mutableListOf<ByteArray>()
        var acceptSecret = true
        var setUp = true
        var canAsk = true

        override fun relayUrl(): String? = "ws://relay.test/relay"
        override suspend fun relayToken(refused: String?): String? {
            tokens += 1
            return "token"
        }
        override fun requestSync() {
            syncs += 1
        }
        override fun canAskForPairing(): Boolean = canAsk
        override suspend fun nigoriIsSetUp(): Boolean {
            setUps += 1
            return setUp
        }
        override suspend fun takeSecret(secret: ByteArray): Boolean {
            secrets.add(secret)
            return acceptSecret
        }
        override fun sayProblem(sentence: String) {
            problems.add(sentence)
        }
    }

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
    private val owner = FakeOwner()
    private val sent = mutableListOf<String>()
    private var socket: FakeSocket? = null
    private val relay = AccountRelay(owner, scope) { _, _, onText, _ ->
        FakeSocket(sent, onText).also { socket = it }
    }

    @After
    fun tearDown() {
        scope.cancel()
    }

    /** Waits until the fake socket exists, as a connection the coroutine opened. Pairing asks
     * only ever run while the app is in front, as the relay's own guard holds. */
    private suspend fun awaitSocket(): FakeSocket {
        withTimeout(5000) {
            while (socket == null) delay(10)
        }
        return socket!!
    }

    private suspend fun activePairing(): FakeSocket {
        relay.setActive(true)
        return awaitSocket()
    }

    @Test
    fun helloOpensAndSyncs() = runTest {
        relay.setActive(true)
        val open = awaitSocket()
        open.onText("""{"type":"hello"}""".encodeToByteArray())
        assertEquals(1, owner.syncs)
        // No pairing was asked for, so hello alone sends nothing.
        assertEquals(0, sent.size)
    }

    @Test
    fun pairingRequestSentOnHello() = runTest {
        relay.requestPairing()
        val open = activePairing()
        open.onText("""{"type":"hello"}""".encodeToByteArray())
        // The request goes once hello says the relay is open.
        assertEquals(1, sent.size)
        assertTrue(sent[0].contains("\"type\":\"pair_request\""))
        assertTrue(sent[0].contains("\"commitment\":\""))
        assertTrue(sent[0].contains("\"os\":\"Android\""))
    }

    @Test
    fun pairingAskNeedsSetUpNigori() = runTest {
        owner.setUp = false
        relay.requestPairing()
        assertEquals(AccountRelay.SET_UP_FIRST, owner.problems.single())
        assertEquals(0, sent.size)
    }

    @Test
    fun acceptThenSealedGivesTheSecret() = runTest {
        relay.requestPairing()
        val open = activePairing()
        open.onText("""{"type":"hello"}""".encodeToByteArray())
        open.onText(
            """{"type":"pair_requested","pairing":"p1","expires_at":9999999999}"""
                .encodeToByteArray())
        // The approver's keys are the test's own, as the other device's would be.
        val approverPrivate = Crypto.randomBytes(32)
        val approver = Crypto.x25519PublicKey(approverPrivate)
        open.onText(
            """{"type":"pair_accept","pairing":"p1","public_key":"${approver.base64URL()}"}"""
                .encodeToByteArray())
        // The code both devices show is made from both keys, so the state carries it.
        val requesterPublic = base64URL(
            sent[1].substringAfter("\"public_key\":\"").substringBefore('"'))!!
        val waiting = relay.pairing.value
        assertTrue(waiting is PairingState.Waiting)
        assertEquals(
            PairingRequest.code(requester = requesterPublic, approver = approver),
            (waiting as PairingState.Waiting).code)
        assertTrue(sent[1].contains("\"type\":\"pair_reveal\""))
        // The approver's side of the seal: its private key with the requester's public one,
        // the shared key both derive, and the pairing's id as the seal's attached data.
        val shared = Crypto.x25519Shared(approverPrivate, requesterPublic)!!
        val sealKey = PairingRequest.sealKey(shared, requesterPublic, approver)
        val secret = ByteArray(RecoveryWords.SECRET_LENGTH) { (it + 1).toByte() }
        val sealedPair = Crypto.chaChaSeal(
            sealKey, ByteArray(12), secret, "p1".encodeToByteArray())!!
        open.onText(
            """{"type":"pair_sealed","pairing":"p1","sealed":"${(sealedPair.first + sealedPair.second).base64URL()}"}"""
                .encodeToByteArray())
        // The secret the approver sealed is the account's now, and the pairing is over.
        assertEquals(1, owner.secrets.size)
        assertArrayEquals(secret, owner.secrets.single())
        assertEquals(PairingState.None, relay.pairing.value)
    }

    @Test
    fun sealedThatDoesNotOpenFailsThePairing() = runTest {
        relay.requestPairing()
        val open = activePairing()
        open.onText("""{"type":"hello"}""".encodeToByteArray())
        open.onText("""{"type":"pair_requested","pairing":"p1"}""".encodeToByteArray())
        val approver = Crypto.x25519PublicKey(Crypto.randomBytes(32))
        open.onText(
            """{"type":"pair_accept","pairing":"p1","public_key":"${approver.base64URL()}"}"""
                .encodeToByteArray())
        // Garbage as the sealed secret: the pairing's open rejects it, and the reason is said.
        open.onText(
            """{"type":"pair_sealed","pairing":"p1","sealed":"${ByteArray(20).base64URL()}"}"""
                .encodeToByteArray())
        assertEquals(PairingState.None, relay.pairing.value)
        assertTrue(owner.problems.contains(AccountRelay.FAILED))
    }

    @Test
    fun cancelSaysSoToTheRelay() = runTest {
        relay.requestPairing()
        val open = activePairing()
        open.onText("""{"type":"hello"}""".encodeToByteArray())
        open.onText("""{"type":"pair_requested","pairing":"p9"}""".encodeToByteArray())
        relay.cancelPairing()
        assertEquals(PairingState.None, relay.pairing.value)
        assertTrue(sent.any { it.contains("\"type\":\"pair_cancel\"") && it.contains("\"p9\"") })
    }

    @Test
    fun refusedPairingShows() = runTest {
        relay.requestPairing()
        val open = activePairing()
        open.onText("""{"type":"hello"}""".encodeToByteArray())
        open.onText("""{"type":"pair_requested","pairing":"p2"}""".encodeToByteArray())
        open.onText(
            """{"type":"pair_closed","pairing":"p2","reason":"cancelled"}""".encodeToByteArray())
        assertEquals(PairingState.Refused, relay.pairing.value)
    }

    @Test
    fun refusedUpgradeRefreshesThenRetriesOnce() = runTest {
        // A socket that refuses the upgrade with 401: the close callback carries the status,
        // which refreshes the token and schedules exactly one retry.
        val refusals = intArrayOf(401, 101)
        var attempt = 0
        var refusedSocket: FakeSocket? = null
        val refusingRelay = AccountRelay(owner, scope) { _, _, onText, onClosed ->
            object : AccountRelay.RelaySocket {
                override fun connect(): Boolean {
                    val status = refusals[attempt]
                    attempt += 1
                    if (status == 101) {
                        refusedSocket = FakeSocket(sent, onText)
                        return refusedSocket!!.connect()
                    }
                    onClosed(status)
                    return false
                }
                override fun sendText(text: String): Boolean = false
                override fun close() {}
            }
        }
        refusingRelay.setActive(true)
        // The first refusal schedules the token refresh, which the fake owner counts.
        withTimeout(5000) {
            while (owner.tokens < 2) delay(10)
        }
        // The refused close path asked for a fresh token once, before its single retry.
        assertEquals(2, owner.tokens)
    }

    @Test
    fun goingInactiveCloses() = runTest {
        relay.setActive(true)
        val open = awaitSocket()
        relay.setActive(false)
        assertEquals(false, open.open)
    }

    @Test
    fun signingOutClosesAnActiveRelay() = runTest {
        relay.setActive(true)
        val open = awaitSocket()
        relay.signedOut()
        assertEquals(false, open.open)
        // A forget keeps the relay closed until the host's word opens it again.
        relay.openRelay()
        val reopened = awaitSocket()
        assertTrue(reopened !== open)
    }

    @Test
    fun signingOutWhilePairingExpiresIt() = runTest {
        relay.setActive(true)
        awaitSocket()
        relay.requestPairing()
        relay.signedOut()
        assertEquals(PairingState.None, relay.pairing.value)
    }
}