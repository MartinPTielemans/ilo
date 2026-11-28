package app.ilo.browser.android.sync

/** One of the account's Spaces, as the account's devices keep them. */
data class SyncedSpace(
    val id: String,
    val name: String,
    val color: Int? = null,
    val icon: String = "",
    val theme: String = ""
)

/** A pinned tab, a folder, or a Today tab, in a Space. */
data class SyncedNode(
    val id: String,
    val kind: Kind,
    val space: String,
    val parent: String? = null,
    val name: String = "",
    val url: String? = null,
    val title: String = "",
    val icon: String = "",
    val seen: Long? = null
) {
    enum class Kind { PIN, FOLDER, TODAY }
}

/** A pairing this phone asked for, which another of the account's devices approves. */
sealed interface PairingState {
    data object None : PairingState
    /** Waiting for another device, which shows the same `code` for the user to compare. */
    data class Waiting(val code: String) : PairingState
    data object Refused : PairingState
}

/** What the last finished sync cycle moved, and when: what the account screen says of the sync. */
data class SyncStats(
    /** When the cycle finished, as the system counts milliseconds. */
    val at: Long,
    /** Entities the account had that this phone took. */
    val downloaded: Int,
    /** Entities this phone had that the account took. */
    val uploaded: Int,
)

/** One open tab of one of the account's other devices, as its last sync reported it. */
data class OtherComputerTab(
    /** The session's tag, the originating computer's cache GUID. */
    val sessionTag: String,
    /** The tab's id on its computer: two tabs can hold one URL, so it's what tells them apart. */
    val tabId: Long,
    /** The computer's human-readable name, from its session header when it gave one. */
    val computerName: String?,
    val url: String,
    val title: String
)

/** What the app sees of the account, and the calls that change it. */
interface IloAccount {
    val state: kotlinx.coroutines.flow.StateFlow<AccountState>
    val spaces: kotlinx.coroutines.flow.StateFlow<List<SyncedSpace>>
    val nodes: kotlinx.coroutines.flow.StateFlow<List<SyncedNode>>
    /** The last thing that went wrong, in a sentence for the user, until something works again. */
    val problem: kotlinx.coroutines.flow.StateFlow<String?>
    /** The signed-in account's email, or empty. */
    val email: kotlinx.coroutines.flow.StateFlow<String>
    /** The pairing this phone asked for, which another of the account's devices approves. */
    val pairing: kotlinx.coroutines.flow.StateFlow<PairingState>
    /** The last finished cycle's outcome, or null before the first one finishes. */
    val stats: kotlinx.coroutines.flow.StateFlow<SyncStats?>
    /** True while a cycle is asking the server, so a request to sync now finds it running. */
    val syncing: kotlinx.coroutines.flow.StateFlow<Boolean>
    /** The open tabs of the account's other devices, from SESSIONS, in their last sync's state. */
    val otherComputers: kotlinx.coroutines.flow.StateFlow<List<OtherComputerTab>>

    suspend fun signIn(code: String): Result<Unit>
    suspend fun signOut()
    /** Stops the sync for good but keeps every synced Space the phone already has, until a
     * sign-in makes it one of the account's devices again. */
    suspend fun disconnect()
    suspend fun refresh()
    /** Opens a Today tab, returning its node id, or null when the account wouldn't take it. */
    suspend fun addTodayTab(space: String, url: String, title: String): String?
    suspend fun removeNode(id: String)
    /** Notes that a tab was shown, at most once an hour, so a look is never a commit. */
    suspend fun sawToday(id: String)
    /** Asks another of the account's devices to hand the account's secret over, and waits. */
    suspend fun requestPairing()
    /** Takes back a pairing that was asked for, which the approver is told of. */
    fun cancelPairing()
    /** The recovery words' suggestion list, for the keyboard. */
    suspend fun recoveryWords(): List<String>?

    companion object {
        /** The default server's address, which the sign-in screen offers. */
        const val DEFAULT_SERVER = "https://sync.ilobrowser.com"
    }
}

sealed interface AccountState {
    data object SignedOut : AccountState
    /** Signed in, but the account's data can't be read until the phone has its secret: the 24
     * recovery words, or a pairing another device approves. */
    data object NeedsSecret : AccountState
    /** The first download of the account's Spaces. */
    data object Starting : AccountState
    data object SignedIn : AccountState
    data object SigningIn : AccountState
    data class Failed(val message: String) : AccountState
}