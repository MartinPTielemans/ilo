package app.ilo.browser.android.browser

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/**
 * A login a page's form was sent with. The password lives in one place only, never printed,
 * logged or mirrored, and goes the moment the offer is saved or dropped.
 */
class UsedLogin(
    val origin: String,
    val username: String,
    /** Held for the offer's life only; empty once it's saved or gone. */
    var password: String,
)

/** What saving a login a form was sent with would do, the account's word. */
enum class LoginKind { SAVE, UPDATE }

/**
 * The offer to save the login a tab's page was just signed in with, or to update the one the
 * account has, ported from the iOS app's LoginSave. All of it is in memory, on the tab: the
 * cache, the history and the phone's settings never get any of it, so closing the tab or
 * quitting forgets it. A form that was sent is no sign-in yet: it becomes an offer once the
 * page has moved on, the page that follows shows no password field, or the form's field left
 * the page it was on.
 */
class LoginSave(private val clock: () -> Long = System::currentTimeMillis) {

    /** How long the chip stands before it goes to the page's sheet row. */
    var standsMillis: Long = 8000
    /** How long it says the password is saved. */
    var savedMillis: Long = 1000

    /** The offer as it stands: null when there is none. */
    val offer = MutableStateFlow<Offer?>(null)

    class Offer(
        val login: UsedLogin,
        val kind: LoginKind,
        /** chip: stands over the bar until [standsMillis] passes; row: waits in the page's
         * sheet; saved: says so for [savedMillis] and is gone, the password already dropped. */
        var phase: Phase,
    ) {
        override fun equals(other: Any?): Boolean = other is Offer && other.login === login && other.kind == kind && other.phase == phase
        override fun hashCode(): Int = login.hashCode() * 31 + kind.hashCode()
    }

    enum class Phase { CHIP, ROW, SAVED }

    /** What an offer for a login would be, or null for none: the account's word. */
    fun interface Judge {
        fun judge(login: UsedLogin): LoginKind?
    }

    /** The login a form was sent with, while it isn't known whether the sign-in took. */
    private var used: UsedLogin? = null
    /** The tab's own frame has gone to another page since that form was sent. */
    private var movedOn = false
    /** The origins the offer stays with: the login's own, and the one it was made on. */
    private var sites: Set<String> = emptySet()
    /** The origin of the page that committed last, if it's a web page. */
    private var here: String? = null

    /** Whether anything of a login is in memory. */
    fun holdsLogin(): Boolean = used != null || offer.value?.login?.password?.isNotEmpty() == true

    /** A form was sent with `login`. It's kept only if saving it would do something. */
    fun used(login: UsedLogin, judge: Judge) {
        used = if (judge.judge(login) == null) null else login
        movedOn = false
    }

    /** The app loads an address in the tab: where the user sends it is no answer to a form. */
    fun loads() {
        used = null
    }

    /** A page committed at `page`; null when it's no web page. */
    fun committed(page: String?) {
        here = page?.let { origin(it) }
        // The offer is for the site the tab was on: elsewhere it's forgotten.
        val standing = offer.value
        if (standing != null && here?.let(sites::contains) != true) drop()
        if (used != null) movedOn = true
    }

    /** The page that committed says whether its own frame shows a password field. */
    fun heard(shows: Boolean, judge: Judge) {
        if (shows) {
            // The form is back on the page that follows it: the sign-in didn't take.
            if (movedOn) used = null
            if (offer.value?.phase == Phase.CHIP) drop()
        } else if (movedOn) {
            val login = used
            used = null
            if (login != null) make(login, judge)
        }
    }

    /** The field the login was read from left its page, in the frame at `origin`. */
    fun left(origin: String, judge: Judge) {
        val login = used ?: return
        if (login.origin != origin || movedOn) return
        used = null
        make(login, judge)
    }

    /** That field was typed in again, or drawn again: the sign-in didn't take. */
    fun edited(origin: String) {
        if (used?.origin == origin && !movedOn) used = null
    }

    private fun make(login: UsedLogin, judge: Judge) {
        // Judged again: the account may have got the login since the form was sent.
        val kind = judge.judge(login) ?: return
        val at = here ?: return
        sites = setOf(login.origin, at)
        offer.value = Offer(login, kind, Phase.CHIP)
        chipSetAt = clock()
    }

    /** When the chip's time runs out; null when nothing waits on time. */
    fun next(): Long? = when (offer.value?.phase) {
        Phase.CHIP -> chipSetAt + standsMillis
        Phase.SAVED -> savedSetAt + savedMillis
        else -> null
    }

    private var chipSetAt = 0L
    private var savedSetAt = 0L

    /** What time does: the chip leaves for the sheet's row, and "Saved" goes. */
    fun tick() {
        val now = clock()
        val standing = offer.value ?: return
        when {
            standing.phase == Phase.CHIP && now >= chipSetAt + standsMillis ->
                setPhase(Phase.ROW)
            standing.phase == Phase.SAVED && now >= savedSetAt + savedMillis -> drop()
        }
    }

    /** The chip leaves for the sheet's row: the user said not now, or the bar went. */
    fun putAway() {
        if (offer.value?.phase == Phase.CHIP) setPhase(Phase.ROW)
    }

    /** The chip's menu is open, so it stays until it's answered. */
    fun hold() {
        if (offer.value?.phase == Phase.CHIP) chipSetAt = Long.MAX_VALUE
    }

    fun forget() = drop()

    /** The account took the login. The password leaves memory before anything is said. */
    fun saved() {
        offer.value?.let { standing ->
            standing.login.password = ""
            standing.phase = Phase.SAVED
            savedSetAt = clock()
            offer.value = standing
        }
    }

    private fun setPhase(phase: Phase) {
        offer.value?.let { it.phase = phase; offer.value = it }
    }

    private fun drop() {
        offer.value?.login?.password = ""
        offer.value = null
    }

    /** A page's origin as its own script reads location.origin, for a web page; null else. */
    companion object {
        fun origin(page: String): String? {
            val url = runCatching { java.net.URI(page) }.getOrNull() ?: return null
            val scheme = url.scheme?.lowercase() ?: return null
            if (scheme != "http" && scheme != "https") return null
            val host = url.host?.lowercase() ?: return null
            if (host.isEmpty()) return null
            val bracketed = if (host.contains(':') && !host.startsWith("[")) "[$host]" else host
            val own = if (scheme == "https") 443 else 80
            val port = url.port.takeIf { it > 0 && it != own }?.let { ":$it" } ?: ""
            return "$scheme://$bracketed$port"
        }
    }
}

/**
 * What a tab's page has said of its sign-in form, and what the user did with the offer,
 * ported from the iOS app's LoginState: the fill offer's half of the login story. All of it
 * is the page's that committed last: the next page starts with none.
 */
class LoginState {
    /** The page's own frame shows a password field. */
    val showsField = MutableStateFlow(false)
    /** The user put the fill offer away, or took it. */
    val putAway = MutableStateFlow(false)
    /** A saved login went into the page's fields. */
    val filled = MutableStateFlow(false)

    /** The script says whether a password field shows; a message on its way from the page
     * before is no word on this one. */
    fun heard(shows: Boolean) {
        showsField.value = shows
    }

    fun committed() {
        showsField.value = false
        putAway.value = false
        filled.value = false
    }

    fun putAwayOffer() {
        putAway.value = true
    }

    fun fills() {
        filled.value = true
    }
}

/** A message from the page's script, in the shape iOS uses. */
sealed class LoginMessage {
    /** The page's own frame shows, or stopped showing, a password field. */
    class Shows(val shows: Boolean) : LoginMessage()
    /** A form was sent with this login. */
    class Used(val username: String, val password: String) : LoginMessage()
    /** The field the login was read from left its page. */
    object Left : LoginMessage()
    /** That field was typed in again, or drawn again: the sign-in didn't take. */
    object Edited : LoginMessage()

    companion object {
        /** Parses one message body, the text after the prefix; null when it's none. */
        fun parse(body: String): LoginMessage? {
            val json = runCatching { Json.parseToJsonElement(body) }.getOrNull() as? JsonObject ?: return null
            return when {
                "shows" in json -> Shows(json["shows"]?.jsonPrimitive?.booleanOrNull ?: return null)
                "login" in json -> {
                    val login = json["login"] as? JsonObject ?: return null
                    Used(login["username"]?.jsonPrimitive?.content ?: "", login["password"]?.jsonPrimitive?.content ?: "")
                }
                json["left"]?.jsonPrimitive?.booleanOrNull == true -> Left
                json["edited"]?.jsonPrimitive?.booleanOrNull == true -> Edited
                else -> null
            }
        }
    }
}
