// The relay: a websocket the server tells the account's devices of each other's changes on,
// and carries a pairing over. Open only while the app is in front, so nothing wakes the phone.
// (Port of AccountRelay.swift.)

package app.ilo.browser.android.sync

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/** A pairing the phone asked for, as the relay sees it. */
internal class PairingAsk {
    val keys = PairingRequest()
    var sent = false
    /** The server's id for it, once it has the request. */
    var id: String? = null
    var approver: ByteArray? = null
    var expiry: Job? = null
}

/** What the relay needs from its owner, which the account supplies. */
internal interface RelayOwner {
    /** The relay's address, or null with no session. */
    fun relayUrl(): String?
    /** An access token that the server still takes, or null if none could be had. */
    suspend fun relayToken(refused: String?): String?
    fun requestSync()
    /** Whether the account has a session and is past needing its secret. */
    fun canAskForPairing(): Boolean
    /** The nigori is set up: another device has a secret worth asking for. */
    suspend fun nigoriIsSetUp(): Boolean
    /** Takes the secret a pairing brought. */
    suspend fun takeSecret(secret: ByteArray): Boolean
    /** The account's word for the user, on whatever went wrong. */
    fun sayProblem(sentence: String)
}

/** The account's side of the relay: the connection and the pairing carried on it. */
internal class AccountRelay(
    private val owner: RelayOwner,
    private val scope: CoroutineScope,
    /** What makes connections, so a test can put a fake one in. */
    private val socketFactory: (url: String, authorization: String,
        onText: (ByteArray) -> Unit, onClosed: (code: Int?) -> Unit) -> RelaySocket,
) {
    interface RelaySocket {
        fun connect(): Boolean
        fun sendText(text: String): Boolean
        fun close()
    }

    private val lock = Object()
    private var socket: RelaySocket? = null
    private var relayOpening = false
    private var relayOpen = false
    private var relayUp: Long? = null
    private var relayRetry: Job? = null
    private var relayDelay = FIRST_DELAY
    private var active = false
    private var ask: PairingAsk? = null

    private val pairingFlow = MutableStateFlow<PairingState>(PairingState.None)
    val pairing: StateFlow<PairingState> = pairingFlow.asStateFlow()

    // MARK: Lifecycle

    fun setActive(next: Boolean) {
        synchronized(lock) {
            if (next == active) return
            active = next
            if (next) {
                openRelay()
            } else {
                closeRelay()
                failPairing(EXPIRED, ifSent = true)
            }
        }
    }

    private fun closeRelay() {
        relayRetry?.cancel()
        relayRetry = null
        socket?.close()
        socket = null
        relayOpen = false
        relayUp = null
        // Leaving was the phone's doing, so the next connection starts from the first delay.
        relayDelay = FIRST_DELAY
    }

    /** The account was forgotten: the socket on the old sign-in goes now, not when the server
     * notices its token is gone, and a pairing waiting in it ends. The host's next word or a
     * sign-in opens it again. */
    fun signedOut() {
        synchronized(lock) {
            closeRelay()
            failPairing(EXPIRED)
        }
    }

    // MARK: The connection

    fun openRelay() {
        synchronized(lock) {
            if (!active || socket != null || relayOpening) return
            relayOpening = true
        }
        relayRetry?.cancel()
        relayRetry = null
        scope.launch {
            val token = owner.relayToken(null)
            synchronized(lock) {
                relayOpening = false
                if (!active || socket != null) return@launch
            }
            val url = owner.relayUrl() ?: return@launch
            if (token == null) {
                retryRelay()
                return@launch
            }
            val next = socketFactory(
                url, "Bearer $token",
                { bytes -> relayFrame(bytes.decodeToString()) },
                { code -> relayClosed(code) })
            if (!next.connect()) {
                // A refusal came through the close callback with the HTTP status, which already
                // refreshed the token and scheduled the retry; scheduling a second one here
                // would race the first with a stale token.
                return@launch
            }
            synchronized(lock) {
                if (!active) {
                    next.close()
                    return@launch
                }
                socket = next
            }
        }
    }

    private fun relayClosed(code: Int?) {
        synchronized(lock) {
            socket = null
            relayOpen = false
            // Only a connection that held earns a quick return. A server that takes one and
            // drops it would otherwise be asked for it every 2 s.
            relayUp?.let { up ->
                if ((System.currentTimeMillis() - up) / 1000 >= 30) relayDelay = FIRST_DELAY
            }
            relayUp = null
            failPairing(EXPIRED, ifSent = true)
        }
        val refused = code == 4001 || code == 401
        if (!refused) {
            retryRelay()
            return
        }
        scope.launch {
            // The server no longer takes the token: another one, or the news that the device
            // was removed, before trying again.
            owner.relayToken(refused = "stale")
            retryRelay()
        }
    }

    private fun retryRelay() {
        synchronized(lock) {
            if (!active || socket != null) return
            val delay = relayDelay
            relayDelay = minOf(relayDelay * 2, LONGEST_DELAY)
            relayRetry?.cancel()
            relayRetry = scope.launch {
                delay(delay * 1000)
                relayRetry = null
                openRelay()
            }
        }
    }

    private val nowSeconds: Double get() = System.currentTimeMillis() / 1000.0

    private fun relaySend(frame: Map<String, String>) {
        val text = jsonOf(frame)
        synchronized(lock) {
            if (!relayOpen) return
            socket?.sendText(text)
        }
    }

    // MARK: Frames

    /** The relay's frames are flat objects of strings and numbers, as the server's answers are. */
    internal fun relayFrame(text: String) {
        val frame = (Json.parse(text) as? Map<*, *>) ?: return
        val type = frame["type"] as? String ?: return
        val id = frame["pairing"] as? String
        when (type) {
            "hello" -> {
                synchronized(lock) {
                    relayOpen = true
                    relayUp = System.currentTimeMillis()
                }
                // Whatever changed while the phone wasn't listening.
                owner.requestSync()
                sendPairingRequest()
            }
            "changed" -> owner.requestSync()
            "pair_requested" -> synchronized(lock) {
                val current = ask ?: return
                if (!current.sent || current.id != null) return
                current.id = id ?: return
                // The server ends a pairing only at its next step, so the phone gives up at
                // its time.
                var left = 300.0
                when (val expires = frame["expires_at"]) {
                    // The JSON reader gives a whole number as Long, a fraction as Double.
                    is Long -> left = minOf(maxOf(expires - nowSeconds, 0.0), 300.0)
                    is Double -> left = minOf(maxOf(expires - nowSeconds, 0.0), 300.0)
                }
                expirePairing((left * 1000).toLong(), EXPIRED)
            }
            "pair_accept" -> synchronized(lock) {
                val current = ask ?: return
                if (id == null || id != current.id) return
                val approver = (frame["public_key"] as? String)?.let { base64URL(it) }
                if (approver == null || approver.size != 32) return
                if (current.approver != null) {
                    // A second key, from someone between the devices trying to make the codes
                    // agree.
                    relaySend(mapOf("type" to "pair_cancel", "pairing" to id!!))
                    failPairing(FAILED)
                    return
                }
                current.approver = approver
                pairingFlow.value = PairingState.Waiting(code = current.keys.code(approver))
                relaySend(
                    mapOf(
                        "type" to "pair_reveal", "pairing" to id!!,
                        "public_key" to current.keys.publicKey.base64URL()))
            }
            "pair_sealed" -> {
                val current = synchronized(lock) { ask ?: return }
                if (id == null || id != current.id) return
                val approver = synchronized(lock) { current.approver } ?: return
                val secret = (frame["sealed"] as? String)?.let { base64URL(it) }
                    ?.let { sealed -> current.keys.open(sealed, approver, id!!) }
                if (secret == null) {
                    failPairing(FAILED)
                    return
                }
                endPairing()
                scope.launch {
                    // The pairing gave a secret, which is the account's only if it opens its
                    // keys.
                    val opened = owner.takeSecret(secret)
                    pairingFlow.value = PairingState.None
                    if (!opened) owner.sayProblem(FAILED)
                }
            }
            "pair_closed" -> synchronized(lock) {
                val current = ask ?: return
                if (id == null || id != current.id) return
                if (frame["reason"] as? String == "cancelled") {
                    endPairing()
                    pairingFlow.value = PairingState.Refused
                } else {
                    failPairing(EXPIRED)
                }
            }
            "error" -> synchronized(lock) {
                val current = ask ?: return
                if (current.id == null || current.id == id) failPairing(FAILED)
            }
            else -> {}
        }
    }

    // MARK: Pairing

    suspend fun requestPairing() {
        if (!owner.canAskForPairing()) return
        if (!owner.nigoriIsSetUp()) {
            owner.sayProblem(SET_UP_FIRST)
            return
        }
        synchronized(lock) {
            endPairingLocked()
            ask = PairingAsk()
            pairingFlow.value = PairingState.Waiting(code = "")
        }
        // The relay gets half a minute to open and take the request.
        expirePairing(30_000, UNREACHABLE)
        openRelay()
        sendPairingRequest()
    }

    fun cancelPairing() {
        synchronized(lock) {
            val id = ask?.id
            if (id != null) relaySend(mapOf("type" to "pair_cancel", "pairing" to id))
            endPairingLocked()
            pairingFlow.value = PairingState.None
        }
    }

    private fun sendPairingRequest() {
        synchronized(lock) {
            val current = ask ?: return
            if (!relayOpen || current.sent) return
            current.sent = true
            relaySend(
                mapOf(
                    "type" to "pair_request",
                    "commitment" to current.keys.commitment.base64URL(),
                    "name" to DEVICE_NAME, "os" to "Android"))
        }
    }

    private fun expirePairing(millis: Long, why: String) {
        val job = scope.launch {
            delay(millis)
            synchronized(lock) {
                val id = ask?.id
                if (id != null) relaySend(mapOf("type" to "pair_cancel", "pairing" to id))
            }
            failPairing(why)
        }
        synchronized(lock) { ask?.expiry = job }
    }

    private fun endPairing() {
        synchronized(lock) { endPairingLocked() }
    }

    private fun endPairingLocked() {
        ask?.expiry?.cancel()
        ask = null
    }

    /** Ends the pairing with a reason. With `ifSent`, only one the relay had been told of,
     * which is lost with the connection; one still waiting for the relay goes on waiting.
     * No pairing asked means no sentence: a sign-out closes the relay, and its word must not
     * read as a pairing that failed. */
    private fun failPairing(why: String, ifSent: Boolean = false) {
        synchronized(lock) {
            val current = ask
            if (current == null) return
            if (ifSent && !current.sent) return
            endPairingLocked()
            pairingFlow.value = PairingState.None
        }
        owner.sayProblem(why)
    }

    companion object {
        const val DEVICE_NAME = "ilo on Android"
        private const val FIRST_DELAY = 2L
        private const val LONGEST_DELAY = 300L
        const val UNREACHABLE =
            "Couldn't reach the account's other devices. Use the recovery words."
        const val EXPIRED =
            "No other device answered in time. Ask again, or use the recovery words."
        const val FAILED =
            "Pairing didn't work. Ask again, or use the recovery words."
        const val SET_UP_FIRST =
            "Set ilo up on a computer first: this account has nothing to read yet."
    }
}

/** The frames the relay takes, always flat strings, in the order the server's protocol names. */
internal fun jsonOf(fields: Map<String, String>): String =
    fields.entries.joinToString(",", prefix = "{", postfix = "}") { (key, value) ->
        "\"" + key.replace("\\", "\\\\").replace("\"", "\\\"") + "\":\"" +
            value.replace("\\", "\\\\").replace("\"", "\\\"") + "\""
    }