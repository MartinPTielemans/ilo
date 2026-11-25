package app.ilo.browser.android.browser

import android.content.Context
import android.content.SharedPreferences
import app.ilo.browser.android.sync.SavedLogin

/**
 * The passwords lane: one login story per tab, between the script's word and the account.
 * A tab's [LoginSave] holds the offer the page's sign-in asks for, its [LoginState] the fill
 * offer the page's form asks for, and the lane answers the script, the engine and the UI.
 * Everything in memory is the tab's own: closing the tab or quitting forgets it, as iOS's
 * does. The never-for list is the phone's own, kept as iOS's settings keep it (one origin per
 * answer), so it holds without an account too.
 */
class PasswordsLane(
    context: Context,
    /** What the account can say of saving and filling; null while there is none. */
    @Volatile private var site: PasswordsSite?,
) {
    /** What the account can say, so a plain unit test stands in for it. */
    interface PasswordsSite {
        /** The logins saved for exactly this page's site, usernames only. */
        fun logins(page: String): List<SavedLogin>
        /** The password of a login, read for this call alone. */
        fun password(login: SavedLogin): String?
        /** What saving a sign-in would do: none, a save, or an update. */
        fun saving(username: String, password: String, page: String): app.ilo.browser.android.sync.LoginSaving
        /** Takes the login in, and says whether it did. */
        fun save(username: String, password: String, page: String): Boolean
    }

    private val prefs: SharedPreferences =
        context.getSharedPreferences("never_passwords", Context.MODE_PRIVATE)

    /** The origins the user said never for, as the offers name their sites. */
    fun neverSites(): Set<String> = prefs.getStringSet("never", emptySet()) ?: emptySet()

    private fun neverAdd(origin: String) {
        prefs.edit().putStringSet("never", neverSites() + origin).apply()
    }

    /** One tab's login story, the save half and the fill half. */
    inner class TabLogins internal constructor() {
        val save = LoginSave()
        val fill = LoginState()

        /** What the script said of the tab's page at `page`. The engine has no word on which
         * frame spoke, so a word from anywhere on the page is taken as the page's own, as
         * iOS's own engine names a frame's site only when it hears one. */
        fun heard(message: LoginMessage, page: String?) {
            val at = page?.let(LoginSave::origin)
            if (at == null) {
                // No page of its own: a form sent before anything committed stands as nothing.
                if (message is LoginMessage.Used) save.loads()
                return
            }
            when (message) {
                is LoginMessage.Shows -> {
                    fill.heard(message.shows)
                    save.heard(message.shows, judge(at))
                }
                is LoginMessage.Used -> save.used(UsedLogin(at, message.username, message.password), judge(at))
                is LoginMessage.Left -> save.left(at, judge(at))
                is LoginMessage.Edited -> save.edited(at)
            }
        }

        /** The offer as it may be shown now: the never-list's word again, since it can change
         * while an offer waits. The sheet's row isn't the chip's business here. */
        fun shownOffer(): LoginSave.Offer? {
            val standing = save.offer.value ?: return null
            if (standing.login.origin in neverSites()) {
                save.forget()
                return null
            }
            return standing
        }

        /** The logins to offer on the page that committed: only ones saved for exactly its site. */
        fun fillOffers(page: String?): List<SavedLogin> {
            if (!fill.showsField.value || fill.putAway.value || page == null) return emptyList()
            val at = LoginSave.origin(page) ?: return emptyList()
            return site?.logins(page)?.filter { it.realm == at } ?: emptyList()
        }

        /** Fills a login into the page. Returns the script to run in it, or null when there's
         * nothing to fill; the caller confirms with the phone's lock first. */
        fun fillScript(login: SavedLogin, page: String?): String? {
            val at = LoginSave.origin(page ?: return null) ?: return null
            if (login.realm != at) return null
            val password = site?.password(login) ?: return null
            fill.fills()
            return LoginScript.fill(login.username, password)
        }

        /** Takes the offer in: the login goes to the account, and the chip says it's saved. */
        fun takeOffer() {
            val standing = save.offer.value ?: return
            val ok = site?.save(standing.login.username, standing.login.password, standing.login.origin + "/") == true
            if (ok) save.saved() else save.forget()
        }

        /** The chip leaves for the page's sheet row: the user said not now. */
        fun putAwayOffer() = save.putAway()

        /** Never for this site: the offer goes, and the site keeps its answer. */
        fun neverSave() {
            save.offer.value?.login?.origin?.let(::neverAdd)
            save.forget()
        }

        private fun judge(at: String) = LoginSave.Judge { login ->
            if (login.password.isEmpty() || login.origin != at) null
            else when (site?.saving(login.username, login.password, login.origin + "/")) {
                app.ilo.browser.android.sync.LoginSaving.SAVE -> LoginKind.SAVE
                app.ilo.browser.android.sync.LoginSaving.UPDATE -> LoginKind.UPDATE
                else -> null
            }
        }
    }

    /** A tab's own story; the same one each call, so its offers survive across the engine's. */
    fun of(tabId: String): TabLogins = lanes.getOrPut(tabId) { TabLogins() }

    private val lanes = java.util.concurrent.ConcurrentHashMap<String, TabLogins>()

    /** The tabs the lane still holds a story for, so a gone one can be found and dropped. */
    fun ids(): Set<String> = lanes.keys.toSet()

    /** A tab is gone: its story goes with it, password and all. */
    fun close(tabId: String) {
        lanes.remove(tabId)?.save?.forget()
    }

    /** The account arrived or left; the offers stand, and ask it again when they're shown. */
    fun setSite(site: PasswordsSite?) {
        this.site = site
    }
}
