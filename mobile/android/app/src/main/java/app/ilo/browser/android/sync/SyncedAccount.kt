// The phone's account: signed in to an ilo server as one more of the account's devices, holding
// the Spaces the others keep and sending its own changes to them (core/sync.cc and
// core/sync_spaces.cc are the computer's). The wire protocol, tokens and Nigori keys are the
// iOS app's (mobile/ios/Sync), in Kotlin.

package app.ilo.browser.android.sync

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.util.UUID

/** The sign-in, kept in the SecretStore. */
class Session(
    var server: String,
    var access: String,
    var expires: Long,
    /** Single-use: each refresh gives the next one. */
    var refresh: String,
    var account: String,
    var device: String,
    var email: String,
    /** What names this device to sync, made once for a sign-in. */
    var cacheGUID: String,
) {
    fun serialized(): String = listOf(
        server, access, expires.toString(), refresh, account, device, email, cacheGUID)
        .joinToString("\n") { it.base64() }

    companion object {
        fun deserialize(bytes: ByteArray): Session? {
            val parts = String(bytes).split("\n")
            if (parts.size != 8) return null
            val plain = parts.map { base64(it) ?: return null }
            return Session(
                String(plain[0]), String(plain[1]),
                String(plain[2]).toLongOrNull() ?: return null,
                String(plain[3]), String(plain[4]), String(plain[5]), String(plain[6]),
                String(plain[7]))
        }
    }
}

/** An entity as the account has it. */
class Record(
    val serverID: String,
    val version: Long,
    val created: Long,
    val hash: String,
    val entity: SpaceEntity,
    /** A newer ilo wrote fields this one doesn't know, which writing it back would lose. */
    val readOnly: Boolean,
    /** The account has a later one this ilo couldn't decrypt or read: still shown, never
     * written, as a change made on it would be made on nothing. */
    var stale: Boolean = false,
) {
    val writable: Boolean get() = !readOnly && !stale
}

/** What the phone changed on an entity and the account hasn't taken yet. */
class Change {
    /** The whole entity, for one the phone made. */
    var created: SpaceEntity? = null
    /** Whether `created` has left in a commit, whose answer may never have come. */
    var sent: Boolean? = null
    // The fields the phone changed, and only those. They stay changed over whatever else
    // another device changes on it meanwhile.
    var name: String? = null
    var url: String? = null
    var title: String? = null
    /** The address of the page `title` is the title of, which is let go once the tab is elsewhere. */
    var page: String? = null
    var seen: Long? = null
    var space: String? = null
    var position: ByteArray? = null
    var deleted: Boolean = false
    /** Tells a change made while its commit was on the way from the one that was sent. */
    var revision: Int = 0

    fun serialized(): String = buildString {
        appendLine(deleted.toString())
        appendLine(revision.toString())
        appendLine(sent?.toString() ?: "-")
        appendLine(created?.serialized()?.base64() ?: "-")
        appendLine(name?.base64() ?: "-")
        appendLine(url?.base64() ?: "-")
        appendLine(title?.base64() ?: "-")
        appendLine(page?.base64() ?: "-")
        appendLine(seen?.toString() ?: "-")
        appendLine(space?.base64() ?: "-")
        appendLine(position?.base64() ?: "-")
    }

    companion object {
        fun deserialize(bytes: ByteArray): Change? {
            val lines = String(bytes).split("\n")
            if (lines.size != 12) return null
            val change = Change()
            change.deleted = lines[0].toBoolean()
            change.revision = lines[1].toIntOrNull() ?: return null
            change.sent = if (lines[2] == "-") null else lines[2].toBoolean()
            change.created = lines[3].takeIf { it != "-" }?.let { text ->
                base64(text)?.let { b -> SpaceEntity.parse(b) }?.first
            }
            change.name = lines[4].takeIf { it != "-" }?.let { b -> String(base64(b) ?: return null) }
            change.url = lines[5].takeIf { it != "-" }?.let { b -> String(base64(b) ?: return null) }
            change.title = lines[6].takeIf { it != "-" }?.let { b -> String(base64(b) ?: return null) }
            change.page = lines[7].takeIf { it != "-" }?.let { b -> String(base64(b) ?: return null) }
            change.seen = lines[8].takeIf { it != "-" }?.toLongOrNull()
            change.space = lines[9].takeIf { it != "-" }?.let { b -> String(base64(b) ?: return null) }
            change.position = lines[10].takeIf { it != "-" }?.let { base64(it) }
            return change
        }
    }
}

/** What the phone has of the account's data, kept in the SyncStateStore. */
class SyncState {
    var birthday: String = ""
    /** How far the phone has each type, by the type's id. */
    var markers: MutableMap<Int, ByteArray> = HashMap()
    var nigori: NigoriSpecifics? = null
    /** By client tag, which is the entity's id. */
    var records: MutableMap<String, Record> = HashMap()
    var changes: MutableMap<String, Change> = HashMap()
    /** The account's password entities by client tag hash, as the server sent them, encrypted. */
    var passwords: MutableMap<String, PasswordRecord> = HashMap()
    /** Logins this phone saved or changed and the account hasn't taken, their ciphertext only. */
    var passwordWrites: MutableMap<String, PasswordWrite> = HashMap()
    /** The account's bookmark entities by client tag hash (the bookmark GUID's own), encrypted. */
    var bookmarks: MutableMap<String, BookmarkRecord> = HashMap()
    /** Bookmarks this phone added or removed and the account hasn't taken, ciphertext only. */
    var bookmarkWrites: MutableMap<String, BookmarkWrite> = HashMap()
    /** Today tabs at an address the account's computers can't take, which stay on this phone. */
    var local: MutableMap<String, SpaceEntity>? = null
    /** Whether all of the account's Spaces have been downloaded once. */
    var downloaded: Boolean = false

    fun serialized(): String = buildString {
        appendLine("ilo-sync-state 1")
        appendLine("birthday " + birthday.base64())
        appendLine("downloaded $downloaded")
        for ((type, token) in markers) appendLine("marker $type " + token.base64())
        nigori?.let { nigori ->
            appendLine(
                "nigori ${nigori.passphraseType} ${nigori.derivationMethod} " +
                    nigori.salt.base64())
            nigori.keybag?.let { appendLine("keybag " + it.keyName.base64() + " " + it.blob.base64()) }
        }
        for ((tag, record) in records) {
            appendLine(
                "record " + tag.base64() + " " + record.serverID.base64() + " ${record.version} " +
                    "${record.created} " + record.hash + " ${record.readOnly} ${record.stale} " +
                    record.entity.serialized().base64())
        }
        for ((tag, change) in changes) {
            appendLine("change " + tag.base64() + " " + change.serialized().base64())
        }
        for ((tag, record) in passwords) {
            appendLine(
                "password " + tag.base64() + " ${record.version} " +
                    (record.id ?: "-").base64() + " " + (record.created?.toString() ?: "-") +
                    " " + record.encrypted.keyName.base64() + " " + record.encrypted.blob.base64())
        }
        for ((tag, write) in passwordWrites) {
            appendLine(
                "passwordwrite " + tag.base64() + " ${write.base} ${write.saved} " +
                    write.encrypted.keyName.base64() + " " + write.encrypted.blob.base64() + " " +
                    write.notes.keyName.base64() + " " + write.notes.blob.base64())
        }
        for ((tag, record) in bookmarks) {
            appendLine(
                "bookmark " + tag.base64() + " ${record.version} " +
                    (record.id ?: "-").base64() + " " + (record.created?.toString() ?: "-") +
                    " " + record.guid.base64() + " " + record.encrypted.keyName.base64() +
                    " " + record.encrypted.blob.base64())
        }
        for ((tag, write) in bookmarkWrites) {
            appendLine(
                "bookmarkwrite " + tag.base64() + " ${write.base} ${write.saved} " +
                    write.position.base64() + " " + (write.encrypted?.keyName?.base64() ?: "-") +
                    " " + (write.encrypted?.blob?.base64() ?: "-"))
        }
        local?.forEach { (tag, entity) ->
            appendLine("local " + tag.base64() + " " + entity.serialized().base64())
        }
    }

    companion object {
        fun deserialize(bytes: ByteArray): SyncState? {
            val state = SyncState()
            val lines = String(bytes).split("\n")
            if (lines.firstOrNull() != "ilo-sync-state 1") return null
            var pendingNigori: NigoriSpecifics? = null
            for (line in lines.drop(1)) {
                if (line.isEmpty()) continue
                val parts = line.split(" ")
                when (parts[0]) {
                    "birthday" -> state.birthday = String(base64(parts[1]) ?: return null)
                    "downloaded" -> state.downloaded = parts[1].toBoolean()
                    "marker" -> {
                        // A torn line, a marker with no token, reads as none: the phone
                        // starts its markers over rather than failing to start at all.
                        val token = parts.getOrNull(2)
                        state.markers[parts[1].toIntOrNull() ?: return null] =
                            if (token == null) ByteArray(0) else base64(token) ?: return null
                    }
                    "nigori" -> {
                        pendingNigori = NigoriSpecifics().apply {
                            passphraseType = parts[1].toIntOrNull() ?: return null
                            derivationMethod = parts[2].toIntOrNull() ?: return null
                            salt = String(base64(parts[3]) ?: return null)
                        }
                    }
                    "keybag" -> pendingNigori?.let {
                        it.keybag = EncryptedData(
                            String(base64(parts[1]) ?: return null),
                            String(base64(parts[2]) ?: return null))
                    }
                    "record" -> {
                        val tag = String(base64(parts[1]) ?: return null)
                        val parsed = SpaceEntity.parse(base64(parts[8]) ?: return null) ?: return null
                        state.records[tag] = Record(
                            String(base64(parts[2]) ?: return null),
                            parts[3].toLongOrNull() ?: return null,
                            parts[4].toLongOrNull() ?: return null,
                            parts[5], parsed.first, parts[6].toBoolean(), parts[7].toBoolean())
                    }
                    "change" -> {
                        val tag = String(base64(parts[1]) ?: return null)
                        state.changes[tag] =
                            Change.deserialize(base64(parts[2]) ?: return null) ?: return null
                    }
                    "password" -> {
                        val tag = String(base64(parts[1]) ?: return null)
                        val encrypted = EncryptedData(
                            String(base64(parts[5]) ?: return null),
                            String(base64(parts[6]) ?: return null))
                        state.passwords[tag] = PasswordRecord(
                            parts[2].toLongOrNull() ?: return null,
                            encrypted,
                            parts[3].takeIf { it != "-" }?.let { b -> String(base64(b) ?: return null) },
                            parts[4].takeIf { it != "-" }?.toLongOrNull())
                    }
                    "passwordwrite" -> {
                        val tag = String(base64(parts[1]) ?: return null)
                        val encrypted = EncryptedData(
                            String(base64(parts[4]) ?: return null),
                            String(base64(parts[5]) ?: return null))
                        val notes = EncryptedData(
                            String(base64(parts[6]) ?: return null),
                            String(base64(parts[7]) ?: return null))
                        state.passwordWrites[tag] = PasswordWrite(
                            encrypted, notes,
                            parts[2].toLongOrNull() ?: return null,
                            parts[3].toLongOrNull() ?: return null)
                    }
                    "bookmark" -> {
                        val tag = String(base64(parts[1]) ?: return null)
                        val encrypted = EncryptedData(
                            String(base64(parts[6]) ?: return null),
                            String(base64(parts[7]) ?: return null))
                        state.bookmarks[tag] = BookmarkRecord(
                            parts[2].toLongOrNull() ?: return null,
                            encrypted,
                            parts[3].takeIf { it != "-" }?.let { b -> String(base64(b) ?: return null) },
                            parts[4].takeIf { it != "-" }?.toLongOrNull(),
                            String(base64(parts[5]) ?: return null))
                    }
                    "bookmarkwrite" -> {
                        val tag = String(base64(parts[1]) ?: return null)
                        val encrypted = if (parts[5] != "-") EncryptedData(
                            String(base64(parts[5]) ?: return null),
                            String(base64(parts[6]) ?: return null)) else null
                        state.bookmarkWrites[tag] = BookmarkWrite(
                            encrypted,
                            parts[2].toLongOrNull() ?: return null,
                            parts[3].toLongOrNull() ?: return null,
                            base64(parts[4]) ?: return null)
                    }
                    "local" -> {
                        val tag = String(base64(parts[1]) ?: return null)
                        val parsed = SpaceEntity.parse(base64(parts[2]) ?: return null) ?: return null
                        if (state.local == null) state.local = HashMap()
                        state.local!![tag] = parsed.first
                    }
                }
            }
            state.nigori = pendingNigori
            return state
        }
    }
}

internal open class SyncFailure(message: String) : Exception(message) {
    class StartOver : SyncFailure("start over")
    class Throttled(val seconds: Int) : SyncFailure("throttled")
}

/** The account as this phone keeps it: the sign-in flow and the Space trees it sees. */
class SyncedAccount(
    private val secrets: SecretStore,
    private val store: SyncStateStore,
    private val deviceName: String = "ilo on Android",
    private val settleMillis: Long = 1000,
    private val wire: Wire? = null,
) : IloAccount {

    private val lock = Object()
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    private val stateFlow = MutableStateFlow<AccountState>(AccountState.SignedOut)
    private val spacesFlow = MutableStateFlow<List<SyncedSpace>>(emptyList())
    private val nodesFlow = MutableStateFlow<List<SyncedNode>>(emptyList())
    private val problemFlow = MutableStateFlow<String?>(null)
    private val emailFlow = MutableStateFlow("")
    private val statsFlow = MutableStateFlow<SyncStats?>(null)
    private val syncingFlow = MutableStateFlow(false)
    private val otherComputersFlow = MutableStateFlow<List<OtherComputerTab>>(emptyList())
    /** SESSIONS state, kept only in memory: the phone never writes any of it, so it reads
     * the account's fresh each run. */
    private val sessionNames = HashMap<String, String?>()
    private val sessionTabs = HashMap<String, OtherComputerTab>()
    /** Each tab's key by its client tag hash, so a deletion takes it out. */
    private val sessionHashes = HashMap<String, String>()

    override val state: StateFlow<AccountState> = stateFlow.asStateFlow()
    override val spaces: StateFlow<List<SyncedSpace>> = spacesFlow.asStateFlow()
    override val nodes: StateFlow<List<SyncedNode>> = nodesFlow.asStateFlow()
    override val problem: StateFlow<String?> = problemFlow.asStateFlow()
    override val email: StateFlow<String> = emailFlow.asStateFlow()
    override val stats: StateFlow<SyncStats?> = statsFlow.asStateFlow()
    override val syncing: StateFlow<Boolean> = syncingFlow.asStateFlow()
    override val otherComputers: StateFlow<List<OtherComputerTab>> = otherComputersFlow.asStateFlow()

    private var session: Session? = null
    private var keys: Keybag? = null
    private var hasSecret = false
    private var syncState = SyncState()
    private var revision = 0
    /** Goes up when the account is forgotten, so what was waiting on the server for the old
     * one does nothing when its answer comes. */
    private var generation = 0
    private var signingIn: Pending? = null
    private var refreshing: Job? = null
    private var syncTask: Job? = null
    private var again = false
    private var settleTask: Job? = null
    private var retryTask: Job? = null
    private var retryDelay = FIRST_DELAY
    private var heldUntil = 0L
    /** The entities in the commit that's on its way. */
    private var sending: Set<String> = emptySet()
    private var client: Server? = null

    /** The relay: the account's changes and pairings arrive on it while the app is in front. */
    private val relay = AccountRelay(relayOwner(), scope) { url, authorization, onText, onClosed ->
        WsRelaySocket(url, authorization, onText, onClosed)
    }

    override val pairing: StateFlow<PairingState> = relay.pairing

    /** The app's own lifecycle tells the relay when to listen, so nothing wakes the phone. */
    fun setActive(active: Boolean) {
        relay.setActive(active)
    }

    private inner class RelayOwnerAdapter : RelayOwner {
        override fun relayUrl(): String? {
            val server = synchronized(lock) { session?.server } ?: return null
            return Server(server).url("/relay")
        }

        override suspend fun relayToken(refused: String?): String? = try {
            accessToken(refused)
        } catch (_: Exception) {
            null
        }

        override fun requestSync() {
            this@SyncedAccount.requestSync()
        }

        override fun canAskForPairing(): Boolean = synchronized(lock) {
            session != null && keys == null
        }

        override suspend fun nigoriIsSetUp(): Boolean {
            if (synchronized(lock) { syncState.nigori } == null) refresh()
            return synchronized(lock) { syncState.nigori?.isSetUp } == true
        }

        override suspend fun takeSecret(secret: ByteArray): Boolean =
            this@SyncedAccount.takeSecret(secret)

        override fun sayProblem(sentence: String) {
            problemFlow.value = sentence
        }
    }

    private fun relayOwner(): RelayOwner = RelayOwnerAdapter()

    init {
        synchronized(lock) {
            val session = Session.deserialize(secrets.read("session") ?: ByteArray(0))
            if (session == null) {
                if (secrets.read(DISCONNECTED) == null) {
                    // State without its sign-in is a backup's: nothing here is signed in.
                    store.erase()
                } else {
                    // A disconnect kept the Spaces while dropping the sign-in, so they come
                    // back here as they were left, with no sign-in and nothing to sync.
                    syncState = SyncState.deserialize(store.load() ?: ByteArray(0)) ?: SyncState()
                    revision = syncState.changes.values.maxOfOrNull { it.revision } ?: 0
                    rebuild()
                }
            } else if (!store.isMarked) {
                // A sign-in this install never made is an earlier install's; a phone the app was
                // deleted from isn't signed in by putting the app back.
                for (name in listOf("session", "secret", "keys")) secrets.write(name, null)
                store.erase()
            } else {
                this.session = session
                emailFlow.value = session.email
                hasSecret = secrets.read("secret") != null
                keys = secrets.read("keys")?.let { String(it) }?.let { Keybag.deserialize(it) }
                syncState = SyncState.deserialize(store.load() ?: ByteArray(0)) ?: SyncState()
                revision = syncState.changes.values.maxOfOrNull { it.revision } ?: 0
                client = Server(session.server, wire)
                updatePhase()
                rebuild()
            }
        }
        requestSync()
    }

    private fun updatePhase() {
        stateFlow.value = when {
            session == null -> AccountState.SignedOut
            keys == null ->
                if (hasSecret) AccountState.Starting else AccountState.NeedsSecret
            else -> if (syncState.downloaded) AccountState.SignedIn else AccountState.Starting
        }
    }

    private fun api(): Server? {
        val session = session ?: return null
        val server = client
        if (server == null || server.base != session.server) {
            client = Server(session.server, wire)
        }
        return client
    }

    /** The address with a scheme, as [beginSignIn]'s page and redirect need one to be
     * compared and loaded as URLs: typed without it, `URI` reads the host as a scheme. */
    private fun schemed(address: String): String =
        if (address.contains("://")) address else "https://$address"

    // MARK: Signing in and out

    /** The server's sign-in page to show, and the address it sends the page on to when it's
     * done. The page runs in the browser UI; its answer's `code` goes to [signIn]. */
    fun beginSignIn(server: String = IloAccount.DEFAULT_SERVER): Pair<String, String> {
        val verifier = Crypto.randomBytes(32).base64URL()
        val state = Crypto.randomBytes(16).base64URL()
        val redirect = Server(schemed(server)).run { url("/signed-in") }
        signingIn = Pending(verifier, state.encodeToByteArray(), schemed(server))
        val page = Server(schemed(server)).run {
            url(
                "/authorize", listOf(
                    "response_type" to "code", "client_id" to "ilo",
                    "redirect_uri" to redirect, "scope" to "openid email offline_access",
                    "state" to state,
                    "code_challenge" to Crypto.sha256(verifier.encodeToByteArray()).base64URL(),
                    "code_challenge_method" to "S256"))
        }
        return page to redirect
    }

    /** Takes the code the server's page answered with, by its query items. */
    override suspend fun signIn(code: String): Result<Unit> {
        // A refused sign-in must say so on the screen, not silently stay signed out.
        val pending = signingIn ?: run {
            problemFlow.value = NO_SIGN_IN
            return Result.failure(IllegalStateException(NO_SIGN_IN))
        }
        val verifier = pending.verifier
        signingIn = null
        // The code comes as "code=...&state=..." or the bare code; the state must match.
        val items = code.split("&").mapNotNull {
            val pair = it.split("=", limit = 2)
            if (pair.size == 2) pair[0] to pair[1] else null
        }.toMap()
        if (items["state"] != null && items["state"] != String(pending.state)) {
            problemFlow.value = NO_SIGN_IN
            return Result.failure(IllegalStateException(NO_SIGN_IN))
        }
        val authorization = items["code"] ?: code
        val server = pending.server
        val redirect = Server(server).run { url("/signed-in") }
        val tokens = try {
            Server(server, wire).token(
                listOf(
                    "grant_type" to "authorization_code", "client_id" to "ilo",
                    "code" to authorization, "redirect_uri" to redirect,
                    "code_verifier" to verifier, "device_name" to deviceName))
        } catch (error: ServerFailure.Refused) {
            problemFlow.value = UNEXPECTED
            return Result.failure(error)
        } catch (error: ServerFailure.Unreachable) {
            problemFlow.value = UNREACHABLE
            return Result.failure(error)
        } catch (error: Exception) {
            problemFlow.value = UNEXPECTED
            return Result.failure(error)
        }
        synchronized(lock) {
            forgetLocked()
            store.mark()
            keep(
                Session(
                    server, tokens.access, tokens.expires, tokens.refresh, tokens.account,
                    tokens.device, tokens.email, Crypto.randomBytes(16).base64()))
            client = Server(server, wire)
            emailFlow.value = tokens.email
            updatePhase()
        }
        // The relay only listens on the host's word, which came before this sign-in, so a
        // fresh sign-in in the foreground would otherwise wait for the next foreground.
        relay.openRelay()
        refresh()
        return Result.success(Unit)
    }

    override suspend fun signOut() {
        val current = synchronized(lock) {
            val session = session ?: return
            forgetLocked()
            session
        }
        relay.signedOut()
        Server(current.server, wire).revoke(current.refresh)
    }

    /** The account's data stays: only the sign-in goes, and with it every way of talking to
     * the server, so nothing of the Spaces this phone already has can change again. */
    override suspend fun disconnect() {
        val current = synchronized(lock) {
            val session = session ?: return
            // The marker goes before the sign-in's secrets go, so a crash between the writes
            // still restores the Spaces rather than erasing them.
            secrets.write(DISCONNECTED, DISCONNECT_MARK)
            generation += 1
            this.session = null
            for (name in listOf("session", "secret", "keys")) secrets.write(name, null)
            signingIn = null
            retryDelay = FIRST_DELAY
            heldUntil = 0
            emailFlow.value = ""
            problemFlow.value = null
            stopWorkLocked()
            updatePhase()
            session
        }
        relay.signedOut()
        Server(current.server, wire).revoke(current.refresh)
    }

    /** Erases the account from the phone: what's in memory, the secrets and the saved state. */
    private fun forgetLocked() {
        generation += 1
        session = null
        keys = null
        hasSecret = false
        syncState = SyncState()
        signingIn = null
        sessionNames.clear()
        sessionTabs.clear()
        sessionHashes.clear()
        otherComputersFlow.value = emptyList()
        for (name in listOf("session", "secret", "keys", DISCONNECTED)) secrets.write(name, null)
        store.erase()
        emailFlow.value = ""
        problemFlow.value = null
        retryDelay = FIRST_DELAY
        heldUntil = 0
        stopWorkLocked()
        updatePhase()
        rebuild()
    }

    /** Stops the old sign-in's work in flight. The scope itself lives on: one account serves
     * the whole process, and a later sign-in needs it working. */
    private fun stopWorkLocked() {
        for (job in listOf(settleTask, retryTask, syncTask, refreshing)) job?.cancel()
        settleTask = null
        retryTask = null
        syncTask = null
        refreshing = null
    }

    /** Saves a sign-in's tokens before the ones they replace are let go of: a refresh token is
     * spent once used, so the new one must not be lost to a crash right after. */
    private fun keep(session: Session) {
        secrets.write("session", session.serialized().encodeToByteArray())
        this.session = session
    }

    /** A token the server takes: the one the phone has, or a new one when that has less than
     * `margin` seconds left or was just refused. */
    private suspend fun accessToken(refused: String? = null, margin: Long = 60): String {
        val current = synchronized(lock) { session } ?: throw IllegalStateException(SIGNED_OUT)
        if (current.access != refused && current.expires > now() + margin) return current.access
        // One refresh at a time, as a second would spend a token the first already did.
        refreshing?.let { it.join(); return synchronized(lock) { session?.access } ?: throw IllegalStateException(SIGNED_OUT) }
        val myGeneration = synchronized(lock) { generation }
        val job = scope.launch {
            val result = runCatching {
                api()?.token(
                    listOf(
                        "grant_type" to "refresh_token", "client_id" to "ilo",
                        "refresh_token" to current.refresh)) ?: throw ServerFailure.Unexpected()
            }
            synchronized(lock) {
                refreshing = null
                // This coroutine stands on its own, so nothing it throws is ever caught: a
                // failed refresh records its word instead, and the caller reads the token the
                // last good word left. Only a server's own refusal signs the device out.
                val session = this@SyncedAccount.session
                if (session == null || myGeneration != generation) return@launch
                result.fold(
                    { tokens ->
                        if (tokens.account != session.account) {
                            problemFlow.value = UNEXPECTED
                            return@launch
                        }
                        session.access = tokens.access
                        session.refresh = tokens.refresh
                        session.expires = tokens.expires
                        session.email = tokens.email
                        keep(session)
                        emailFlow.value = tokens.email
                    },
                    { error ->
                        if (error is ServerFailure.Refused) {
                            // The server's own word that the device was removed: nothing else
                            // it says signs out.
                            forgetLocked()
                            problemFlow.value = REMOVED
                        } else if (error is ServerFailure.Unreachable) {
                            problemFlow.value = UNREACHABLE
                        } else {
                            problemFlow.value = UNEXPECTED
                        }
                    })
            }
        }
        refreshing = job
        job.join()
        return synchronized(lock) { session?.access } ?: throw IllegalStateException(SIGNED_OUT)
    }

    // MARK: The secret

    /** Keeps the account's secret if it opens the account's keys, and starts on its data. */
    private suspend fun takeSecret(secret: ByteArray): Boolean {
        val nigori = synchronized(lock) { syncState.nigori } ?: return false
        if (secret.size != RecoveryWords.SECRET_LENGTH) return false
        val myGeneration = synchronized(lock) { generation }
        // The words themselves are the passphrase the keys are derived from, which takes a moment.
        val passphrase = RecoveryWords.words(secret)
        val opened = withContext(Dispatchers.Default) { nigori.open(passphrase) }
        if (myGeneration != synchronized(lock) { generation } || opened == null) return false
        synchronized(lock) {
            secrets.write("secret", secret)
            secrets.write("keys", opened.serialized().encodeToByteArray())
            keys = opened
            hasSecret = true
            updatePhase()
        }
        requestSync()
        return true
    }

    override suspend fun requestPairing() {
        relay.requestPairing()
    }

    override fun cancelPairing() {
        relay.cancelPairing()
    }

    /** The 24 recovery words, for Settings, when the phone has the secret. */
    override suspend fun recoveryWords(): List<String>? {
        val secret = synchronized(lock) {
            if (!hasSecret) return null else secrets.read("secret")
        } ?: return null
        return RecoveryWords.words(secret).split(" ")
    }

    /** The words the user types, when the phone doesn't have the secret yet. */
    suspend fun enterRecoveryWords(words: String): Result<Unit> {
        val secret = RecoveryWords.secret(words) ?: return Result.failure(IllegalArgumentException(WRONG_WORDS))
        synchronized(lock) { if (session == null) return Result.failure(IllegalStateException(SIGNED_OUT)) }
        // A nigori the phone read before the computer set the account up stays a placeholder in
        // the store, so it is asked for again before its answer is trusted.
        if (synchronized(lock) { syncState.nigori }?.isSetUp != true) refresh()
        val nigori = synchronized(lock) { syncState.nigori }
        if (nigori == null || !nigori.isSetUp) {
            problemFlow.value = SET_UP_FIRST
            return Result.failure(IllegalStateException(SET_UP_FIRST))
        }
        if (!takeSecret(secret)) return Result.failure(IllegalArgumentException(WRONG_WORDS))
        refresh()
        return Result.success(Unit)
    }

    // MARK: Reading

    /** The Spaces in the sidebar's order. */
    private fun ordered(entities: List<SpaceEntity>): List<SpaceEntity> =
        entities.map { it to UniquePosition.fromProto(it.position ?: ByteArray(0)) }
            .sortedWith { a, b ->
                if (a.second.isValid != b.second.isValid) {
                    return@sortedWith if (a.second.isValid) -1 else 1
                }
                if (a.second != b.second) {
                    return@sortedWith if (a.second.lessThan(b.second)) -1 else 1
                }
                (a.first.id ?: "").compareTo(b.first.id ?: "")
            }
            .map { it.first }

    /** An entity as the app shows it: the account's, or one the phone made, with what the
     * phone changed on it. */
    private fun effectiveLocked(tag: String): SpaceEntity? {
        val change = syncState.changes[tag]
        if (change?.deleted == true) return null
        var entity = change?.created ?: syncState.records[tag]?.entity ?: syncState.local?.get(tag)
            ?: return null
        if (change != null) {
            change.name?.let { entity.name = it }
            change.url?.let { entity.url = it }
            change.title?.let { entity.title = it }
            change.seen?.let { seen -> if (isLater(seen, entity.seen)) entity.seen = seen }
            change.space?.let { entity.space = it }
            change.position?.let { entity.position = it }
        }
        return entity
    }

    /** A showing is told to the account only an hour on from the last it has, as the
     * computer's is (kSeenStep), or every look at a tab would be a commit. */
    private fun isLater(seen: Long, known: Long?): Boolean = seen >= (known ?: 0) + 3600

    /** Remakes what the app reads and saves the state. After every change to either. */
    private fun changed() {
        rebuild()
        if (session != null) {
            runCatching { store.save(syncState.serialized().encodeToByteArray()) }
        }
    }

    private fun rebuild() {
        val tags = (syncState.records.keys + syncState.changes.keys +
            (syncState.local?.keys ?: emptySet())).toSet()
        val entities = ArrayList<SpaceEntity>()
        for (tag in tags) {
            val entity = effectiveLocked(tag) ?: continue
            // A kind this ilo doesn't know isn't one to show as the default kind, a Space.
            if (entity.kind == null && syncState.records[tag]?.readOnly == true) continue
            entities.add(entity)
        }
        val order = ordered(entities)
        val spaces = order.filter { (it.kind ?: SpaceEntity.Kind.SPACE) == SpaceEntity.Kind.SPACE }
            .map {
                SyncedSpace(
                    it.id ?: "", it.name ?: "", it.color?.toInt(), it.icon ?: "", it.theme ?: "")
            }
        val known = spaces.map { it.id }.toSet()
        val nodes = LinkedHashMap<String, SyncedNode>()
        for (entity in order) {
            val id = entity.id ?: continue
            val kind = entity.kind ?: continue
            if (kind == SpaceEntity.Kind.SPACE) continue
            val space = entity.space ?: continue
            if (id !in known && space !in known) continue
            val seen = entity.seen
            nodes[id] = SyncedNode(
                id = id,
                kind = if (kind == SpaceEntity.Kind.TODAY) SyncedNode.Kind.TODAY
                    else if (kind == SpaceEntity.Kind.FOLDER) SyncedNode.Kind.FOLDER
                    else SyncedNode.Kind.PIN,
                space = space,
                parent = entity.parent,
                name = entity.name ?: "",
                url = entity.url,
                title = entity.title ?: "",
                icon = entity.icon ?: "",
                seen = seen,
            )
        }
        spacesFlow.value = spaces
        nodesFlow.value = nodes.values.toList()
    }

    // MARK: Changes

    private fun change(tag: String, body: (Change) -> Unit) {
        val change = syncState.changes[tag] ?: Change()
        body(change)
        revision += 1
        change.revision = revision
        syncState.changes[tag] = change
    }

    private fun now(): Long = System.currentTimeMillis() / 1000

    /** A page's address as a tab's entity has it, for the pages the account takes: http or
     * https with a host of letters, digits, "-", "_" and "." (or a bracketed IPv6, or four
     * plain numbers), and a port from 1 to 65535. Stricter than Chromium's GURL, since one
     * refused here only stays on the phone, and one wrongly sent is deleted for every device. */
    private fun address(url: String): String? {
        val schemeCut = url.indexOf("://")
        if (schemeCut == -1) return null
        val scheme = url.substring(0, schemeCut).lowercase()
        if (scheme != "http" && scheme != "https") return null
        var text = url
        val authorityEnd = url.indexOfAny(charArrayOf('/', '?', '#'), schemeCut + 3)
            .let { if (it == -1) url.length else it }
        val authority = url.substring(schemeCut + 3, authorityEnd)
        val host = authority.substringAfterLast('@')
        var port: String? = null
        var name = host
        if (host.startsWith("[")) {
            val close = host.indexOf(']')
            if (close == -1) return null
            val after = host.substring(close + 1)
            if (after.isNotEmpty() && !after.startsWith(":")) return null
            if (after.isNotEmpty()) port = after.substring(1)
            name = host.substring(0, close + 1)
        } else if (host.lastIndexOf(':') != -1) {
            port = host.substring(host.lastIndexOf(':') + 1)
            name = host.substring(0, host.lastIndexOf(':'))
        }
        if (port != null) {
            if (port.length > 5 || port.isEmpty() || port.any { it < '0' || it > '9' }) return null
            val number = port.toIntOrNull() ?: return null
            if (number < 1 || number > 65535) return null
        }
        if (name.startsWith("[")) {
            // A bracketed IPv6 literal; this port reads it as a name of the allowed bytes and
            // lets the computers decide, which is what they do for one they can open.
            val inner = name.substring(1, name.length - 1)
            if (inner.isEmpty() || inner.any {
                    val c = it.code
                    !(c in 0x30..0x39 || c in 0x41..0x5a || c in 0x61..0x7a ||
                        c == 0x2d || c == 0x2e || c == 0x5f || c == ':'.code)
                }
            ) return null
        } else {
            val named = name.all {
                it in '0'..'9' || it in 'A'..'Z' || it in 'a'..'z' || it == '-' || it == '.' || it == '_'
            }
            var labels = name.split(".")
            if (labels.size > 1 && labels.last().isEmpty()) labels = labels.dropLast(1)
            if (!named || labels.isEmpty() || labels.any { it.isEmpty() }) return null
            val last = labels.last()
            // GURL reads a host whose last label is a number as an IPv4 address: only four
            // plain ones.
            val numeric = last.all { it in '0'..'9' } || last.lowercase().startsWith("0x")
            if (numeric && !(labels.size == 4 && labels.all { label ->
                    label.length <= 3 && (label == "0" || !label.startsWith("0")) &&
                        label.all { it in '0'..'9' } && (label.toIntOrNull() ?: 256) <= 255
                })
            ) return null
        }
        // As Chromium writes an address with no path.
        if (authorityEnd == url.length) text = "$url/"
        if (text.toByteArray(Charsets.UTF_8).size >= 2 shl 20) return null
        return text
    }

    /** A page the phone opened, as a Today tab of `space`, first in it. A tab at an address the
     * account doesn't take is this phone's alone. */
    override suspend fun addTodayTab(space: String, url: String, title: String): String? {
        val id = SpaceEntity.TODAY_PREFIX + UUID.randomUUID().toString().lowercase()
        val writable = synchronized(lock) { spacesFlow.value.any { it.id == space } }
        if (!writable) {
            problemFlow.value = NOT_A_PAGE
            return null
        }
        val took = address(url)
        synchronized(lock) {
            if (took == null) {
                val tab = SpaceEntity()
                tab.id = id
                tab.kind = SpaceEntity.Kind.TODAY
                tab.space = space
                tab.url = url
                tab.title = title
                tab.seen = now()
                if (syncState.local == null) syncState.local = HashMap()
                syncState.local!![id] = tab
                changed()
                return id
            }
            val tab = SpaceEntity()
            tab.id = id
            tab.kind = SpaceEntity.Kind.TODAY
            tab.space = space
            tab.position = firstTodayIn(space).proto()
            tab.title = title
            tab.url = took
            tab.seen = now()
            change(id) { it.created = tab }
        }
        wrote()
        return id
    }

    /** Notes a showing only an hour on from the last the account has, as the computer's is
     * (kSeenStep), so a look at a tab is never a commit. */
    override suspend fun sawToday(id: String) {
        val at = now()
        synchronized(lock) {
            if (effectiveLocked(id)?.kind != SpaceEntity.Kind.TODAY) return
            val seen = effectiveLocked(id)?.seen
            if (!isLater(at, seen)) return
            if (syncState.local?.contains(id) == true) {
                syncState.local?.get(id)?.seen = at
                changed()
                return
            }
            if (syncState.records[id]?.writable == false) {
                problemFlow.value = NEWER
                return
            }
            editLocked(id) { it.seen = at }
        }
        wrote()
    }

    private fun firstTodayIn(space: String): UniquePosition {
        val first = nodesFlow.value.firstOrNull { it.space == space && it.kind == SyncedNode.Kind.TODAY }
            ?.let { node -> synchronized(lock) { effectiveLocked(node.id) } }
        return UniquePosition.before(
            first?.position?.let { UniquePosition.fromProto(it) } ?: UniquePosition(),
            UniquePosition.randomSuffix())
    }

    private fun lastPinnedIn(space: String): UniquePosition {
        val last = nodesFlow.value.filter {
            it.space == space && it.kind == SyncedNode.Kind.PIN && it.parent == null
        }.lastOrNull()?.let { node -> synchronized(lock) { effectiveLocked(node.id) } }
        return UniquePosition.after(
            last?.position?.let { UniquePosition.fromProto(it) } ?: UniquePosition(),
            UniquePosition.randomSuffix())
    }

    /** Deletes a Today tab, a pinned tab or a folder, as its kind is. */
    override suspend fun removeNode(id: String) {
        val tag = if (id.startsWith(SpaceEntity.TODAY_PREFIX)) id else SpaceEntity.key(id)
        val isToday = synchronized(lock) { effectiveLocked(id)?.kind == SpaceEntity.Kind.TODAY }
        if (isToday && synchronized(lock) { syncState.local?.contains(id) } == true) {
            synchronized(lock) {
                syncState.local?.remove(id)
                changed()
            }
            return
        }
        synchronized(lock) {
            if (syncState.records[id]?.writable == false) {
                problemFlow.value = NEWER
                return
            }
            change(tag) { it.deleted = true }
        }
        wrote()
    }

    /** Renames a Space or a folder; the account's computers never take a tab's name. */
    suspend fun rename(id: String, name: String) {
        synchronized(lock) {
            val entity = effectiveLocked(id) ?: return
            val kind = entity.kind ?: SpaceEntity.Kind.SPACE
            if (kind != SpaceEntity.Kind.SPACE && kind != SpaceEntity.Kind.FOLDER) return
            if (syncState.records[id]?.writable == false) {
                problemFlow.value = NEWER
                return
            }
            if ((entity.name ?: "") == name) return
            editLocked(id) { it.name = name }
        }
        wrote()
    }

    private fun editLocked(tag: String, body: (SpaceEntity) -> Unit) {
        val before = effectiveLocked(tag) ?: return
        val wanted = before
        body(wanted)
        if (wanted == before) return
        change(tag) { change ->
            if (change.created != null && change.sent != true) {
                change.created = wanted
            } else {
                if (wanted.name != before.name) change.name = wanted.name
                if (wanted.url != before.url) change.url = wanted.url
                if (wanted.title != before.title || wanted.url != before.url) {
                    change.title = wanted.title
                    change.page = wanted.url
                }
                if (wanted.seen != before.seen) change.seen = wanted.seen
                if (wanted.space != before.space) {
                    change.space = wanted.space
                    change.position = wanted.position
                }
            }
        }
        if (tag !in sending) rebaseLocked(tag)
    }

    /** Fits a pending change to what the account has, after either changed: a field that's the
     * account's own by now is let go, and so is a title whose page the tab has left. A change
     * with nothing left is removed. */
    private fun rebaseLocked(tag: String) {
        var change = syncState.changes[tag] ?: return
        if (change.deleted) return
        if (change.created != null && change.sent != true) return
        val known = change.created ?: syncState.records[tag]?.entity ?: run {
            syncState.changes.remove(tag)
            return
        }
        if (change.name == known.name) change.name = null
        if (change.url == known.url) change.url = null
        if (change.title != null &&
            (change.page != (change.url ?: known.url) || change.title == known.title)
        ) change.title = null
        if (change.title == null) change.page = null
        change.seen?.let { seen -> if (!isLater(seen, known.seen)) change.seen = null }
        if (change.space == known.space) {
            change.space = null
            change.position = null
        }
        if (change.created == null && change.name == null && change.url == null &&
            change.title == null && change.seen == null && change.space == null
        ) {
            syncState.changes.remove(tag)
        }
        syncState.changes[tag] = change
        change = syncState.changes[tag] ?: return
        Unit
    }

    /** Shows the change now and sends it once no other has followed it for a moment, so a page
     * that redirects twice and then gets its title is one commit. */
    private fun wrote() {
        synchronized(lock) { changed() }
        settleTask?.cancel()
        settleTask = scope.launch {
            delay(settleMillis)
            settleTask = null
            requestSync()
        }
    }

    // MARK: The sync cycle

    /** Runs a cycle, or one more after the cycle that's running. */
    fun requestSync() {
        synchronized(lock) { if (session == null) return }
        val running = syncTask
        if (running != null && running.isActive) {
            again = true
            return
        }
        syncTask = scope.launch {
            try {
                syncingFlow.value = true
                while (true) {
                    again = false
                    cycle()
                    if (!again) break
                }
            } finally {
                // A disconnect cancels the cycle mid-flight: the flag goes down with it.
                syncingFlow.value = false
                syncTask = null
            }
        }
    }

    /** Asks the server for what changed. */
    override suspend fun refresh() {
        settleTask?.cancel()
        settleTask = null
        requestSync()
        syncTask?.join()
    }

    private suspend fun cycle() {
        val myGeneration = synchronized(lock) { generation }
        try {
            var said = false
            var later = false
            var got = 0
            var sent = 0
            for (pass in 1..3) {
                got += download(myGeneration)
                if (synchronized(lock) { keys } == null) {
                    // The account's keys changed, or its data was cleared: the secret the phone
                    // has opens the new ones, or it's another account's by now.
                    if (hasSecret && synchronized(lock) { syncState.nigori?.isSetUp } == true) {
                        val secret = secrets.read("secret")
                        if (secret != null) {
                            val opened = takeSecret(secret)
                            if (myGeneration != synchronized(lock) { generation }) return
                            if (!opened) {
                                secrets.write("secret", null)
                                synchronized(lock) {
                                    hasSecret = false
                                    updatePhase()
                                }
                                problemFlow.value = WORDS_CHANGED
                                said = true
                            }
                        }
                    }
                    break
                }
                val result = commit(myGeneration, last = pass == 3)
                said = said || result.conflicted
                later = result.later
                sent += result.sent
                if (!result.later) break
            }
            synchronized(lock) {
                if (myGeneration != generation) return
                retryDelay = FIRST_DELAY
                if (keys == null && syncState.nigori?.isSetUp != true) {
                    problemFlow.value = SET_UP_FIRST
                } else if (!said) {
                    problemFlow.value = null
                }
                // What the screen says of the sync is the last cycle that finished, so a failed
                // one leaves the last good word standing.
                statsFlow.value = SyncStats(System.currentTimeMillis(), got, sent)
            }
            if (later) retryLater(myGeneration)
        } catch (error: Exception) {
            if (myGeneration != synchronized(lock) { generation }) return
            when (error) {
                is SyncFailure.StartOver -> synchronized(lock) {
                    // The sign-in itself may be the change to start over from: a cycle that
                    // begins after a sign-out has the new generation and nothing to sync.
                    if (session == null) return
                    syncState = SyncState()
                    keys = null
                    secrets.write("keys", null)
                    updatePhase()
                    changed()
                    again = true
                }
                is SyncFailure.Throttled -> {
                    // At least 5 s and at most an hour, as the iOS app clamps the server's word.
                    val wait = error.seconds.coerceIn(5, 3600)
                    heldUntil = now() + wait
                    retryAfter(wait * 1000L)
                }
                is ServerFailure.Unreachable -> {
                    problemFlow.value = UNREACHABLE
                    retryLater(myGeneration)
                }
                is IllegalStateException -> Unit
                else -> {
                    problemFlow.value = UNEXPECTED
                    retryLater(myGeneration)
                }
            }
        }
    }

    /** Tries again by itself only while something waits on it: a change to send, or the first
     * download. Without the relay, nothing else says when there's something to get. */
    private fun retryLater(myGeneration: Int) {
        synchronized(lock) {
            if (myGeneration != generation) return
            if (syncState.changes.isEmpty() && stateFlow.value != AccountState.Starting) return
        }
        retryAfter(retryDelay * 1000)
        retryDelay = minOf(retryDelay * 2, LONGEST_DELAY)
    }

    private fun retryAfter(millis: Long) {
        val wait = maxOf(millis, (heldUntil - now()) * 1000)
        retryTask?.cancel()
        retryTask = scope.launch {
            delay(wait)
            retryTask = null
            requestSync()
        }
    }

    /** Sends one message. An access token the server refuses is refreshed once. */
    private suspend fun send(message: ByteArray, myGeneration: Int): SyncResponse {
        val cacheGUID = synchronized(lock) { session?.cacheGUID } ?: throw SyncFailure.StartOver()
        val server = api() ?: throw SyncFailure.StartOver()
        val access = accessToken()
        var response = try {
            server.command(message, access, cacheGUID)
        } catch (error: ServerFailure.Unauthorized) {
            val fresh = accessToken(refused = access)
            server.command(message, fresh, cacheGUID)
        }
        if (myGeneration != synchronized(lock) { generation }) throw SyncFailure.StartOver()
        when (response.error) {
            SyncResponse.ErrorCode.SUCCESS -> Unit
            SyncResponse.ErrorCode.NOT_MY_BIRTHDAY, SyncResponse.ErrorCode.OBSOLETE ->
                throw SyncFailure.StartOver()
            SyncResponse.ErrorCode.THROTTLED ->
                throw SyncFailure.Throttled(response.throttle ?: 60)
            else -> throw ServerFailure.Unexpected()
        }
        synchronized(lock) {
            if (syncState.birthday.isEmpty()) {
                syncState.birthday = response.birthday
            } else if (response.birthday.isNotEmpty() && response.birthday != syncState.birthday) {
                throw SyncFailure.StartOver()
            }
        }
        return response
    }

    private suspend fun download(myGeneration: Int): Int {
        var got = 0
        val email = synchronized(lock) { session?.email } ?: throw SyncFailure.StartOver()
        while (true) {
            // The Spaces only once the phone can read them, so none is passed by undecrypted.
            val askedSpaces = synchronized(lock) { keys != null }
            val types = if (askedSpaces)
                listOf(DataType.NIGORI, DataType.ILO_SPACE, DataType.PASSWORDS,
                    DataType.BOOKMARKS, DataType.SESSIONS)
                else listOf(DataType.NIGORI)
            val before = synchronized(lock) { HashMap(syncState.markers) }
            val markers = types.map { it to (before[it] ?: ByteArray(0)) }
            val response = send(
                SyncMessage.getUpdates(
                    share = email, birthday = synchronized(lock) { syncState.birthday },
                    markers = markers),
                myGeneration)
            got += response.updates.size
            synchronized(lock) { applyLocked(response, asked = types) }
            if (response.remaining <= 0 ||
                synchronized(lock) { syncState.markers == before.toMap() }
            ) break
        }
        synchronized(lock) {
            if (keys != null) syncState.downloaded = true
            updatePhase()
            changed()
        }
        return got
    }

    private fun applyLocked(response: SyncResponse, asked: List<Int>) {
        // The Nigori first: the keys it names are the ones the rest is encrypted with.
        for (update in response.updates) {
            val bytes = update.nigori ?: continue
            val nigori = NigoriSpecifics(bytes)
            if (nigori == this.syncState.nigori) continue
            syncState.nigori = nigori
            val keys = keys
            if (keys != null && (!nigori.isSetUp || nigori.keybag?.keyName != keys.defaultName)) {
                this.keys = null
                secrets.write("keys", null)
            }
        }
        for ((type, token) in response.markers) {
            if (type == DataType.NIGORI) syncState.markers[type] = token
        }
        val keys = keys ?: return
        if (DataType.ILO_SPACE !in asked) return
        val tags = HashMap<String, String>()
        for ((tag, record) in syncState.records) tags[record.hash] = tag
        // An entity the phone made and sent may be the account's without the phone having heard.
        for ((tag, change) in syncState.changes) {
            if (change.created != null && change.sent == true) tags[clientTagHash(tag = tag)] = tag
        }
        for (update in response.updates) {
            if (!update.isSpace || update.serverTag != null) continue
            val hash = update.hash ?: continue
            val known = tags[hash]
            // Chromium's processor takes an update only if it's newer than what it has.
            if (known != null) {
                val record = syncState.records[known]
                if (record != null && update.version <= record.version) continue
            }
            if (update.deleted) {
                if (known == null) continue
                syncState.records.remove(known)
                tags.remove(hash)
                // What the phone changed on it has nothing left to change. One it made again
                // stays, unless that was sent: then this is the account's word after it.
                val change = syncState.changes[known]
                if (change != null && (change.created == null || change.sent == true)) {
                    syncState.changes.remove(known)
                }
                continue
            }
            // As the computer's bridge: an entity that isn't encrypted, isn't valid or isn't
            // under its own id's hash is passed over.
            val encrypted = update.encrypted
            val plain = if (encrypted != null) keys.decrypt(encrypted) else null
            val parsed = plain?.let { protoFields(it)?.last(DataType.ILO_SPACE)?.bytes }
                ?.let { protoFields(it) }?.last(1)?.bytes
                ?.let { SpaceEntity.parse(it) }
            if (encrypted == null || plain == null || parsed == null || !parsed.first.isValid ||
                parsed.first.id == null || clientTagHash(tag = parsed.first.id!!) != hash
            ) {
                // The marker moves past it all the same, so what the phone has of it is no
                // longer what the account has, and a change made on that could only be refused.
                if (known != null) syncState.records[known]?.stale = true
                continue
            }
            syncState.records[parsed.first.id!!] = Record(
                update.id, update.version, update.created, hash, parsed.first, !parsed.second)
            tags[hash] = parsed.first.id!!
            heardLocked(parsed.first.id!!)
        }
        for ((type, token) in response.markers) {
            if (type == DataType.ILO_SPACE) syncState.markers[type] = token
        }
        applyPasswordsLocked(response)
        applyBookmarksLocked(response)
        applySessionsLocked(response)
    }

    /** The SESSIONS entities of an answer: each computer's header names it, and its tabs land
     * as they are, since the phone never writes any of them. A header and its tabs may come
     * in different answers, so the names stay across them. */
    private fun applySessionsLocked(response: SyncResponse) {
        var changed = false
        for (update in response.updates) {
            if (!update.isSessions) continue
            val hash = update.hash ?: continue
            if (update.deleted) {
                sessionHashes.remove(hash)?.let { key ->
                    if (sessionTabs.remove(key) != null) changed = true
                }
                continue
            }
            val encrypted = update.encrypted ?: continue
            val keys = keys ?: return
            val parsed = keys.decrypt(encrypted)
                ?.let { SessionEntity.parse(it) } ?: continue
            if (parsed.isHeader) {
                sessionNames[parsed.sessionTag] = parsed.clientName
                changed = true
            } else {
                val url = parsed.url ?: continue
                val key = "${parsed.sessionTag}/${parsed.tabId}"
                sessionTabs[key] =
                    OtherComputerTab(
                        parsed.sessionTag,
                        parsed.tabId ?: continue,
                        sessionNames[parsed.sessionTag],
                        url,
                        parsed.title ?: "")
                sessionHashes[hash] = key
                changed = true
            }
        }
        for ((key, tab) in sessionTabs) {
            sessionNames[tab.sessionTag]?.let { name ->
                sessionTabs[key] = tab.copy(computerName = name)
            }
        }
        if (changed) {
            otherComputersFlow.value = sessionTabs.values.sortedBy { it.sessionTag }
        }
        for ((type, token) in response.markers) {
            if (type == DataType.SESSIONS) syncState.markers[type] = token
        }
    }

    /** The password entities of an answer, whatever isn't one this phone can keep passed over
     * (Passwords.swift's applyPasswords). Deletion is by hash alone, since a deletion may come
     * without its type and a hash is of one type only. */
    private fun applyPasswordsLocked(response: SyncResponse) {
        var changed = false
        for (update in response.updates) {
            if (update.serverTag != null) continue
            val hash = update.hash ?: continue
            if (update.deleted) {
                if (syncState.passwords.remove(hash) != null) changed = true
                continue
            }
            if (!update.isPassword) continue
            val known = syncState.passwords[hash]
            if (known != null && update.version <= known.version && known.id != null) continue
            // One with no ciphertext isn't an account's with a passphrase: what the phone had of
            // it is no longer the account's, so that goes too.
            val record = update.password?.let {
                PasswordRecord(update.version, it, update.id.ifEmpty { null }, update.created)
            }
            if (record == null) {
                if (syncState.passwords.remove(hash) != null) changed = true
            } else {
                syncState.passwords[hash] = record
                changed = true
            }
        }
        for ((type, token) in response.markers) {
            if (type == DataType.PASSWORDS) syncState.markers[type] = token
        }
        if (changed) changed()
    }

    /** The bookmark entities of an answer. Only a page in the mobile folder is kept: a folder,
     * a page under another one, or what can't be read passes over, and what the phone had of
     * it goes too, as the phone never writes over what it can't read. A deletion always wins
     * over a pending resurrection, the desktop's rule. */
    private fun applyBookmarksLocked(response: SyncResponse) {
        var changed = false
        for (update in response.updates) {
            if (update.serverTag != null) continue
            val hash = update.hash ?: continue
            if (update.deleted) {
                if (syncState.bookmarks.remove(hash) != null) changed = true
                // Deletion wins over resurrection: a pending add of the same page goes.
                if (syncState.bookmarkWrites.remove(hash) != null) changed = true
                continue
            }
            if (!update.isBookmark) continue
            val known = syncState.bookmarks[hash]
            if (known != null && update.version <= known.version && known.id != null) continue
            val encrypted = update.bookmark
            val plain = if (encrypted != null) keys?.decrypt(encrypted) else null
            val data = plain?.let(::BookmarkData)
            if (encrypted == null || plain == null || data == null || !data.takes) {
                if (syncState.bookmarks.remove(hash) != null) changed = true
                continue
            }
            syncState.bookmarks[hash] = BookmarkRecord(
                update.version, encrypted, update.id.ifEmpty { null }, update.created, data.guid)
            changed = true
        }
        for ((type, token) in response.markers) {
            if (type == DataType.BOOKMARKS) syncState.markers[type] = token
        }
        if (changed) {
            bookmarksTick.value += 1
            changed()
        }
    }

    /** The account said what it has of an entity: in a download, or in a commit's answer. */
    private fun heardLocked(tag: String) {
        val change = syncState.changes[tag] ?: return
        if (change.created != null && change.sent == true) {
            change.created = null
            change.sent = null
        }
        rebaseLocked(tag)
    }

    private class Sent(val tag: String, val revision: Int, val entity: SpaceEntity?,
        val entry: CommitEntry)

    /** A sign-in waiting for the server's answer: the verifier its code must match, the
     * state the page was made with, and the server the code goes back to. */
    private class Pending(val verifier: String, val state: ByteArray, val server: String)

    /** The entries the pending changes come to. A change that changes nothing by now is dropped. */
    private fun planLocked(keys: Keybag): List<Sent> {
        val sent = ArrayList<Sent>()
        val time = protoTime()
        // By node, so a tab's delete under one id and its put under the other are side by side.
        val tags = syncState.changes.keys.sortedWith { a, b ->
            val keyA = SpaceEntity.key(a)
            val keyB = SpaceEntity.key(b)
            if (keyA != keyB) keyA.compareTo(keyB) else a.compareTo(b)
        }
        for (tag in tags) {
            val change = syncState.changes[tag] ?: continue
            val record = syncState.records[tag]
            val entity = effectiveLocked(tag)
            // Nothing to send: it's as the account has it, or deleted before the account got it.
            if (record?.writable == false || entity == record?.entity) {
                syncState.changes.remove(tag)
                continue
            }
            val entry = CommitEntry(
                // Chromium sends a new entity under a random id, which the server replaces.
                id = record?.serverID ?: UUID.randomUUID().toString().lowercase(),
                hash = record?.hash ?: clientTagHash(tag = tag),
                version = record?.version ?: 0,
                created = record?.created ?: time, modified = time,
                encrypted = if (entity != null) {
                    val known = record?.entity
                    // A Today tab that differs only in its title isn't sent for that alone, as
                    // the computer doesn't: a title goes with the next real change.
                    if (known != null && entity.kind == SpaceEntity.Kind.TODAY &&
                        !known.title.isNullOrEmpty()
                    ) {
                        val same = SpaceEntity().apply {
                            id = entity.id; kind = entity.kind; space = entity.space
                            parent = entity.parent; position = entity.position
                            name = entity.name; url = entity.url; title = known.title
                            color = entity.color; icon = entity.icon; theme = entity.theme
                            seen = entity.seen
                        }
                        if (same == known) continue
                    }
                    keys.encrypt(CommitEntry.plaintext(entity))
                } else {
                    null
                })
            sent.add(Sent(tag, change.revision, entity, entry))
        }
        return sent
    }

    /** What one pass of a cycle did: whether another download is needed, whether the user was
     * told something, and how many entities went each way. */
    private class Pass(
        val conflicted: Boolean,
        val undone: Boolean,
        val later: Boolean,
        val sent: Int,
    )

    /** `last` when there's no download left after it in this cycle. */
    private suspend fun commit(myGeneration: Int, last: Boolean): Pass {
        var conflicted = false
        var undone = false
        var later = false
        var took = 0
        val email: String
        val pending: List<Sent>
        synchronized(lock) {
            val keys = keys ?: return Pass(false, false, false, 0)
            if (settleTask != null) return Pass(false, false, false, 0)
            if (now() < heldUntil) {
                later = syncState.changes.isNotEmpty()
                return Pass(false, false, later, 0)
            }
            email = session?.email ?: return Pass(false, false, false, 0)
            pending = planLocked(keys)
        }
        var at = 0
        while (at < pending.size) {
            // Chromium's batch is 25. A pinned or unpinned tab's two entries go in one, as the
            // computer moves the tab only when it gets both at once, and closes it otherwise.
            var end = minOf(at + 25, pending.size)
            if (end < pending.size &&
                SpaceEntity.key(pending[end].tag) == SpaceEntity.key(pending[end - 1].tag)
            ) end += 1
            val batch = pending.subList(at, end)
            at = end
            var marked = false
            synchronized(lock) {
                for (entry in batch) {
                    val change = syncState.changes[entry.tag]
                    if (change?.created != null && change.sent != true) {
                        change.sent = true
                        marked = true
                    }
                }
                if (marked) changed()
                sending = batch.map { it.tag }.toSet()
            }
            val response = try {
                send(
                    SyncMessage.commit(
                        share = email,
                        birthday = synchronized(lock) { syncState.birthday },
                        cacheGUID = synchronized(lock) { session?.cacheGUID ?: "" },
                        entries = batch.map { it.entry }),
                    myGeneration)
            } catch (error: Exception) {
                synchronized(lock) {
                    sending = emptySet()
                    for (entry in batch) rebaseLocked(entry.tag)
                }
                throw error
            }
            synchronized(lock) {
                sending = emptySet()
                if (response.entries.size != batch.size) throw ServerFailure.Unexpected()
                for ((entry, answer) in batch.zip(response.entries)) {
                    when (answer.result) {
                        SyncResponse.EntryResult.SUCCESS -> {
                            took += 1
                            if (entry.entity != null) {
                                syncState.records[entry.tag] = Record(
                                    answer.id.ifEmpty { entry.entry.id }, answer.version,
                                    entry.entry.created, entry.entry.hash, entry.entity, false)
                            } else {
                                syncState.records.remove(entry.tag)
                            }
                            // A change made since it was sent goes with the next commit.
                            val change = syncState.changes[entry.tag]
                            if (change?.revision == entry.revision) {
                                syncState.changes.remove(entry.tag)
                            } else {
                                heardLocked(entry.tag)
                            }
                        }
                        SyncResponse.EntryResult.CONFLICT -> if (!last) {
                            // Another device changed it first: the next download brings what it
                            // has now, and the change is made on that.
                            conflicted = true
                        } else {
                            syncState.changes.remove(entry.tag)
                            problemFlow.value = UNDONE
                            undone = true
                        }
                        SyncResponse.EntryResult.INVALID -> {
                            syncState.changes.remove(entry.tag)
                            problemFlow.value = UNDONE
                            undone = true
                        }
                        SyncResponse.EntryResult.OVER_QUOTA, SyncResponse.EntryResult.TRANSIENT,
                        null -> later = true
                    }
                }
                for (entry in batch) rebaseLocked(entry.tag)
                changed()
            }
        }
        val passwords = commitPasswords(myGeneration, last)
        val bookmarks = commitBookmarks(myGeneration, last)
        return Pass(
            conflicted || passwords.conflicted || bookmarks.conflicted,
            undone || passwords.undone || bookmarks.undone,
            later || passwords.later || bookmarks.later,
            took + passwords.sent + bookmarks.sent)
    }

    /** A pending password write as it stands against what the account has now, or null for one
     * there's nothing left to send of (Passwords.swift's settled). */
    private fun settledPasswordLocked(
        hash: String, keys: Keybag, undone: BooleanArray
    ): PasswordWrite? {
        var write = syncState.passwordWrites[hash] ?: return null
        val mine = keys.decrypt(write.encrypted)?.let(::PasswordForm)
        if (mine == null) {
            // Under a key the account no longer has: nothing of it can be sent.
            syncState.passwordWrites.remove(hash)
            undone[0] = true
            return null
        }
        val record = syncState.passwords[hash]
        if (record != null && record.version != write.base) {
            if (record.encrypted == write.encrypted) {
                syncState.passwordWrites.remove(hash)
                return null
            }
            val known = keys.decrypt(record.encrypted)?.let(::PasswordForm)
            if (known == null || known.realm != mine.realm || known.username != mine.username) {
                syncState.passwordWrites.remove(hash)
                undone[0] = true
                return null
            }
            if (known.password == mine.password) {
                syncState.passwordWrites.remove(hash)
                return null
            }
            // Another device changed the login meanwhile: the user's new password goes on what
            // it is now, keeping everything else the computer has.
            val saved = write.saved
            val data = keys.decrypt(record.encrypted)
                ?.let { PasswordData.changed(it, mine.password, saved) }
            val again = data?.let { PasswordWrite.of(it, keys, record.version, saved) }
            if (again == null) {
                syncState.passwordWrites.remove(hash)
                undone[0] = true
                return null
            }
            write = again
        } else if (record == null && write.base != 0L) {
            // Deleted on another device since: the user's save makes it again.
            write = PasswordWrite(write.encrypted, write.notes, 0, write.saved)
        }
        if (write.encrypted.keyName != keys.defaultName) {
            // The account's key changed since: under the one its computers encrypt with now.
            val plain = keys.decrypt(write.encrypted) ?: return null
            write = PasswordWrite.of(plain, keys, write.base, write.saved) ?: return null
        }
        if (write != syncState.passwordWrites[hash]) {
            syncState.passwordWrites[hash] = write
            changed()
        }
        return write
    }

    /** Sends the logins this phone saved, in their own commit after the Spaces' (Passwords.swift's
     * commitPasswords). */
    private suspend fun commitPasswords(
        myGeneration: Int, last: Boolean
    ): Pass {
        var conflicted = false
        var undone = false
        var later = false
        var took = 0
        val email: String
        val batch: List<Triple<String, PasswordWrite, CommitEntry>>
        synchronized(lock) {
            if (syncState.passwordWrites.isEmpty()) return Pass(false, false, false, 0)
            val keys = keys ?: return Pass(false, false, false, 0)
            if (settleTask != null || now() < heldUntil) {
                return Pass(false, false, syncState.passwordWrites.isNotEmpty(), 0)
            }
            email = session?.email ?: return Pass(false, false, false, 0)
            val pending = ArrayList<Triple<String, PasswordWrite, CommitEntry>>()
            val undoneFlag = booleanArrayOf(false)
            for (hash in syncState.passwordWrites.keys.sorted()) {
                val write = settledPasswordLocked(hash, keys, undoneFlag) ?: continue
                val record = syncState.passwords[hash]
                // A record kept by an ilo before this one has no id, until the download that follows.
                if (record != null && record.id == null) {
                    later = true
                    continue
                }
                val now = protoTime()
                val entry = CommitEntry(
                    id = record?.id ?: UUID.randomUUID().toString().lowercase(),
                    hash = hash,
                    version = record?.version ?: 0,
                    created = record?.created ?: write.saved, modified = now,
                    encrypted = null, password = write)
                pending.add(Triple(hash, write, entry))
            }
            if (undoneFlag[0]) problemFlow.value = PASSWORD_NOT_SAVED
            batch = pending
        }
        if (batch.isEmpty()) return Pass(conflicted, undone, later, took)
        val response = send(
            SyncMessage.commit(
                share = email,
                birthday = synchronized(lock) { syncState.birthday },
                cacheGUID = synchronized(lock) { session?.cacheGUID ?: "" },
                entries = batch.map { it.third }),
            myGeneration)
        synchronized(lock) {
            if (response.entries.size != batch.size) throw ServerFailure.Unexpected()
            for ((sent, answer) in batch.zip(response.entries)) {
                when (answer.result) {
                    SyncResponse.EntryResult.SUCCESS -> {
                        took += 1
                        syncState.passwords[sent.first] = PasswordRecord(
                            answer.version, sent.second.encrypted,
                            answer.id.ifEmpty { sent.third.id }, sent.third.created)
                        // One saved again since it was sent goes with the next commit.
                        if (syncState.passwordWrites[sent.first] == sent.second) {
                            syncState.passwordWrites.remove(sent.first)
                        } else {
                            again = true
                        }
                    }
                    SyncResponse.EntryResult.CONFLICT -> if (!last) {
                        // The account has one this phone hasn't seen: the next download brings it.
                        conflicted = true
                    } else {
                        syncState.passwordWrites.remove(sent.first)
                        problemFlow.value = PASSWORD_NOT_SAVED
                        undone = true
                    }
                    SyncResponse.EntryResult.INVALID -> {
                        syncState.passwordWrites.remove(sent.first)
                        problemFlow.value = PASSWORD_NOT_SAVED
                        undone = true
                    }
                    SyncResponse.EntryResult.OVER_QUOTA, SyncResponse.EntryResult.TRANSIENT,
                        null -> later = true
                }
            }
            changed()
        }
        return Pass(conflicted, undone, later, took)
    }

    /** Sends the bookmarks this phone changed, in their own commit after the passwords'. A
     * pending write stands against what the account has now: one the account gained meanwhile
     * goes unless the account's is the same page, and one the account lost means the phone's
     * add starts over at version 0. */
    private suspend fun commitBookmarks(
        myGeneration: Int, last: Boolean
    ): Pass {
        var conflicted = false
        var undone = false
        var later = false
        var took = 0
        val email: String
        val batch: List<Triple<String, BookmarkWrite, CommitEntry>>
        synchronized(lock) {
            if (syncState.bookmarkWrites.isEmpty()) return Pass(false, false, false, 0)
            val keys = keys ?: return Pass(false, false, false, 0)
            if (settleTask != null || now() < heldUntil) {
                return Pass(false, false, syncState.bookmarkWrites.isNotEmpty(), 0)
            }
            email = session?.email ?: return Pass(false, false, false, 0)
            val pending = ArrayList<Triple<String, BookmarkWrite, CommitEntry>>()
            for (hash in syncState.bookmarkWrites.keys.sorted()) {
                var write = syncState.bookmarkWrites[hash] ?: continue
                val record = syncState.bookmarks[hash]
                if (record != null && record.version != write.base) {
                    val known = keys.decrypt(record.encrypted)?.let(::BookmarkData)
                    if (known == null || known.url != write.keptUrl(keys)) {
                        // Another page under the same name now: this phone's change is nothing.
                        syncState.bookmarkWrites.remove(hash)
                        undone = true
                        continue
                    }
                    // The same page: the change goes on what the account has.
                    write = BookmarkWrite(write.encrypted, record.version, write.saved, write.position)
                } else if (record == null && write.base != 0L) {
                    // Deleted on another device since: the phone's add makes it again.
                    write = BookmarkWrite(write.encrypted, 0, write.saved, write.position)
                }
                if (write.encrypted != null && write.encrypted.keyName != keys.defaultName) {
                    // The account's key changed since: under the one its computers encrypt with now.
                    val plain = keys.decrypt(write.encrypted!!) ?: continue
                    val re = keys.encrypt(plain) ?: continue
                    write = BookmarkWrite(re, write.base, write.saved, write.position)
                }
                if (write != syncState.bookmarkWrites[hash]) syncState.bookmarkWrites[hash] = write
                if (record != null && record.id == null) {
                    later = true
                    continue
                }
                val now = protoTime()
                val entry = CommitEntry(
                    id = record?.id ?: UUID.randomUUID().toString().lowercase(),
                    hash = hash,
                    version = write.base,
                    created = record?.created ?: write.saved, modified = now,
                    encrypted = null,
                    bookmark = write,
                    folder = false)
                pending.add(Triple(hash, write, entry))
            }
            batch = pending
        }
        if (batch.isEmpty()) return Pass(conflicted, undone, later, took)
        val response = send(
            SyncMessage.commit(
                share = email,
                birthday = synchronized(lock) { syncState.birthday },
                cacheGUID = synchronized(lock) { session?.cacheGUID ?: "" },
                entries = batch.map { it.third }),
            myGeneration)
        synchronized(lock) {
            if (response.entries.size != batch.size) throw ServerFailure.Unexpected()
            for ((sent, answer) in batch.zip(response.entries)) {
                when (answer.result) {
                    SyncResponse.EntryResult.SUCCESS -> {
                        took += 1
                        val sentEncrypted = sent.second.encrypted
                        if (sentEncrypted != null) {
                            val plain = keys?.decrypt(sentEncrypted)
                            val data = plain?.let(::BookmarkData)
                            if (data != null) {
                                syncState.bookmarks[sent.first] = BookmarkRecord(
                                    answer.version, sentEncrypted,
                                    answer.id.ifEmpty { sent.third.id }, sent.third.created, data.guid)
                            }
                        } else {
                            syncState.bookmarks.remove(sent.first)
                        }
                        // One changed again since it was sent goes with the next commit.
                        if (syncState.bookmarkWrites[sent.first] == sent.second) {
                            syncState.bookmarkWrites.remove(sent.first)
                        } else {
                            again = true
                        }
                    }
                    SyncResponse.EntryResult.CONFLICT -> if (!last) {
                        conflicted = true
                    } else {
                        syncState.bookmarkWrites.remove(sent.first)
                        undone = true
                    }
                    SyncResponse.EntryResult.INVALID -> {
                        syncState.bookmarkWrites.remove(sent.first)
                        undone = true
                    }
                    SyncResponse.EntryResult.OVER_QUOTA, SyncResponse.EntryResult.TRANSIENT,
                        null -> later = true
                }
            }
            bookmarksTick.value += 1
            changed()
        }
        return Pass(conflicted, undone, later, took)
    }

    /** The account's saved passwords, for the UI's offers and fills. */
    val passwords = Passwords(this)

    // MARK: Bookmarks

    /** The account's bookmarks, for the lane that moves them to the local store. */
    val bookmarks = Bookmarks(this)

    /** Goes up whenever what the account has for bookmarks changed, so the lane looks once. */
    val bookmarksTick = MutableStateFlow(0)

    internal fun keysForPasswords(): Keybag? = synchronized(lock) {
        if (stateFlow.value == AccountState.SignedIn) keys else null
    }

    internal fun passwordsReady(): Boolean = keysForPasswords() != null

    /** A login's ciphertext as this phone has it now: what it wrote and hasn't sent, over what
     * the account has. */
    internal fun passwordCiphertext(hash: String): EncryptedData? = synchronized(lock) {
        syncState.passwordWrites[hash]?.encrypted ?: syncState.passwords[hash]?.encrypted
    }

    /** Every login this phone could read, saved and unsent. */
    internal fun passwordHashes(): Set<String> = synchronized(lock) {
        syncState.passwords.keys + syncState.passwordWrites.keys
    }

    /** The realm a login saved at `page` goes under, if it's one this phone can save for. */
    internal fun savableRealm(page: String, password: String): String? {
        if (!passwordsReady() || password.isEmpty()) return null
        val realm = PasswordForm.realm(page) ?: return null
        return if (PasswordTag.takes(realm)) realm else null
    }

    /** The name of a login the account hasn't got, if nothing is under it yet. Whatever is there
     * and isn't a login to fill is something this phone can't read, and never writes over. */
    internal fun unusedPasswordName(realm: String, username: String): String? {
        val secret = synchronized(lock) {
            if (!passwordsReady()) null else secrets.read("secret")
        } ?: return null
        if (secret.size != RecoveryWords.SECRET_LENGTH) return null
        val hash = PasswordTag.hash(secret, realm, username)
        return if (passwordCiphertext(hash) == null) hash else null
    }

    /** Keeps a login to send with the next commit: its ciphertext only, never its words. */
    internal fun savePassword(hash: String, data: ByteArray, now: Long): Boolean {
        var wrote = false
        synchronized(lock) {
            val keys = keysForPasswords() ?: return false
            val write = PasswordWrite.of(data, keys, syncState.passwords[hash]?.version ?: 0, now)
                ?: return false
            syncState.passwordWrites[hash] = write
            wrote = true
            changed()
        }
        if (wrote) requestSync()
        return true
    }

    /** The account's bookmarks ready to read and change, as the lane asks. */
    internal fun bookmarksReady(): Boolean = keysForPasswords() != null

    /** A page's address as a bookmark's entity has it, for the ones the account takes: the
     * same rule a Today tab's address follows. */
    internal fun bookmarkAddress(url: String): String? = address(url)

    /** Every bookmark entity this phone has, saved and unsent, by client tag hash. */
    internal fun bookmarkHashes(): Set<String> = synchronized(lock) {
        syncState.bookmarks.keys + syncState.bookmarkWrites.filterValues { it.encrypted != null }.keys
    }

    /** A bookmark's data as this phone has it now: what it wrote and hasn't sent, over what
     * the account has. */
    internal fun bookmarkData(hash: String): BookmarkData? = synchronized(lock) {
        val keys = keysForPasswords()
        val encrypted = syncState.bookmarkWrites[hash]?.encrypted
            ?: syncState.bookmarks[hash]?.encrypted
        encrypted?.let { keys?.decrypt(it) }?.let(::BookmarkData)
    }

    /** Keeps a bookmarked page to send with the next commit: its ciphertext only. */
    internal fun saveBookmark(url: String, title: String): Boolean {
        var wrote = false
        synchronized(lock) {
            val keys = keysForPasswords() ?: return false
            val guid = UUID.randomUUID().toString().lowercase()
            val data = BookmarkData.new(url, title, guid, System.currentTimeMillis())
            val encrypted = keys.encrypt(data) ?: return false
            val position = lastBookmarkIn(keys).proto()
            syncState.bookmarkWrites[BookmarkData.tagHash(guid)] =
                BookmarkWrite(encrypted, syncState.bookmarks[BookmarkData.tagHash(guid)]?.version ?: 0,
                    protoTime(), position)
            wrote = true
            changed()
        }
        if (wrote) requestSync()
        return wrote
    }

    /** A page's bookmark goes, sent as a deletion with the next commit. A deletion always
     * wins over a resurrection, so one the account doesn't have yet is still sent: its
     * computers would bring it back otherwise. */
    internal fun removeBookmark(url: String): Boolean {
        var wrote = false
        synchronized(lock) {
            val keys = keysForPasswords() ?: return false
            val entry = syncState.bookmarks.entries.firstOrNull {
                keys.decrypt(it.value.encrypted)?.let(::BookmarkData)?.url == url
            } ?: return false
            val hash = entry.key
            val record = entry.value
            syncState.bookmarks.remove(hash)
            syncState.bookmarkWrites[hash] = BookmarkWrite(
                null, record.version, protoTime(), ByteArray(0))
            wrote = true
            bookmarksTick.value += 1
            changed()
        }
        if (wrote) requestSync()
        return wrote
    }

    /** Where a new bookmark sits among the folder's own ones, after the last it has. */
    private fun lastBookmarkIn(keys: Keybag): UniquePosition {
        var last: UniquePosition? = null
        for (record in syncState.bookmarks.values) {
            val data = keys.decrypt(record.encrypted)?.let(::BookmarkData) ?: continue
            val position = data.position?.let { UniquePosition.fromProto(it) } ?: continue
            if (!position.isValid) continue
            if (last == null || last.lessThan(position)) last = position
        }
        return UniquePosition.after(last ?: UniquePosition(), UniquePosition.randomSuffix())
    }

    companion object {
        const val SET_UP_FIRST =
            "Set ilo up on a computer first: this account has nothing to read yet."
        const val REMOVED = "This phone was signed out of the account."
        const val UNEXPECTED = "The server answered with something ilo doesn't understand."
        const val UNDONE = "The account didn't take a change, so it was undone."
        const val NEWER = "A newer ilo made this, and this one can't change it."
        const val WORDS_CHANGED = "The account's recovery words changed. Enter the new ones."
        const val NOT_A_PAGE = "Only web pages can be tabs of the account."
        const val PASSWORD_NOT_SAVED =
            "ilo couldn't save this password to your account. It's still offered in this page's menu."
        const val NO_SIGN_IN = "The sign-in page answered for a sign-in this phone didn't start."
        const val WRONG_WORDS = "Those aren't this account's recovery words."
        const val SIGNED_OUT = "This phone is signed out."
        const val UNREACHABLE = "Couldn't reach the server."

        /** The name of the secret that marks a disconnect: its kept Spaces a restart
         * restores instead of erasing. */
        private const val DISCONNECTED = "disconnected"
        private val DISCONNECT_MARK = byteArrayOf(1)

        // A retry waits 2 s, then twice as long each time up to 5 min, as the computer's does.
        const val FIRST_DELAY = 2L
        const val LONGEST_DELAY = 300L
    }
}