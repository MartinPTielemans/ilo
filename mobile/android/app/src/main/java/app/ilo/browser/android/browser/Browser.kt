package app.ilo.browser.android.browser

import android.content.Context
import android.content.SharedPreferences
import android.content.pm.PackageManager
import android.Manifest
import androidx.core.content.ContextCompat
import app.ilo.browser.android.R
import app.ilo.browser.android.blocking.EasyListBlocker
import java.io.File
import kotlinx.coroutines.launch
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.MainScope

/** Ad blocking the user chose, in front of the engine that applies it: one store, the engine's. */
class BlockingSettings(private val blocker: EasyListBlocker, private val scope: kotlinx.coroutines.CoroutineScope) {
    /** How the last Add went, so the screen can say it: null until one runs. */
    val listStatus = kotlinx.coroutines.flow.MutableStateFlow<String?>(null)
    /** The sites blocking is off for, as a flow so a sheet's row hears a turn of its switch. */
    val sitesOffFlow = kotlinx.coroutines.flow.MutableStateFlow(blocker.siteExceptions())

    /** Blocking on or off, so every surface's switch agrees with the others. */
    val isOnFlow = kotlinx.coroutines.flow.MutableStateFlow(blocker.isOn)

    fun toggle(on: Boolean) {
        blocker.setOn(on)
        isOnFlow.value = on
    }

    val isOn: Boolean get() = blocker.isOn

    /** The sites blocking is off for, by the name one owner registers. */
    fun sitesOff(): List<String> = blocker.siteExceptions()

    /** Whether the page at `url` is blocked on its own site's choice. */
    fun siteOff(url: String): Boolean = blocker.siteBlockingOff(url)

    /** The addresses of the filter lists the user added. */
    fun customListUrls(): List<String> = blocker.customListUrls()

    /** Whether a kind's rules run. */
    fun kindOn(kind: EasyListBlocker.Kind): Boolean = blocker.kindOn(kind)

    /** Turns a kind's rules on or off, the engine rebuilt with it. */
    fun setKind(kind: EasyListBlocker.Kind, on: Boolean) = blocker.setKind(kind, on)

    /** The built-in lists a kind runs on, by the names the Settings screen shows. */
    fun builtInLists(kind: EasyListBlocker.Kind): List<String> = blocker.builtInListNames(kind)

    /** Re-fetches the built-in lists and refits the engine; the screen says how it went.
     * Runs on the browser's own scope so leaving the screen mid-fetch changes nothing. */
    fun updateLists() {
        listStatus.value = "update"
        scope.launch {
            val result = blocker.updateBuiltIns()
            listStatus.value = if (result.isSuccess) null else "update failed"
        }
    }

    fun setSite(site: String, blocks: Boolean) {
        blocker.setSiteBlockingOff("https://$site/", !blocks)
        sitesOffFlow.value = blocker.siteExceptions()
    }

    /** Fetches the list, keeping it only when the engine could load it: a wrong address tells
     * the user and never lands in the store. Runs on the browser's own scope so leaving the
     * screen mid-fetch changes nothing. */
    fun addList(url: String) {
        // Android's URL accepts only lowercase schemes, so a pasted "HTTP://" is stored
        // in the canonical form that will actually fetch.
        val trimmed = url.trim()
        val schemeEnd = trimmed.indexOf("://")
        val fixed = if (schemeEnd > 0) {
            trimmed.substring(0, schemeEnd).lowercase() + trimmed.substring(schemeEnd)
        } else trimmed
        listStatus.value = fixed
        scope.launch {
            blocker.addCustomListUrl(fixed)
            if (blocker.refreshCustomLists().isSuccess) {
                listStatus.value = null
            } else {
                blocker.removeCustomListUrl(fixed)
                listStatus.value = "$fixed failed"
            }
        }
    }

    fun removeList(url: String) {
        blocker.removeCustomListUrl(url)
        // A failure here keeps the last good engine; the user asked for removal either way.
        scope.launch { blocker.refreshCustomLists() }
    }
}

/** What the phone keeps for itself: where a search goes and whether the UI is dark. */
class BrowserSettings(val prefs: SharedPreferences) {
    /** The UI recomposes when this changes; the activity collects it for IloTheme. */
    val darkModeFlow = kotlinx.coroutines.flow.MutableStateFlow(darkMode)

    var searchEngine: SearchEngine
        get() = engineNamed(prefs.getString("searchEngine", null))
        set(value) { prefs.edit().putString("searchEngine", value.id).apply() }

    /** The engines the user added, as the owner asked, beside the ones ilo ships with: a
     * JSON list under its own key, so the shipped ones stay what they are and the user's own
     * can go and come without touching them. */
    var customEngines: List<SearchEngine>
        get() {
            val stored = prefs.getString("customEngines", null) ?: return emptyList()
            val engines = runCatching {
                kotlinx.serialization.json.Json.decodeFromString<List<SearchEngine>>(stored)
            }.getOrNull() ?: return emptyList()
            return engines.filter { it.id.isNotBlank() && it.name.isNotBlank() && it.prefix.startsWith("http") }
        }
        set(value) {
            val stored = kotlinx.serialization.json.Json.encodeToString(value)
            prefs.edit().putString("customEngines", stored).apply()
        }

    /** Every engine the field can pick: the shipped ones then the user's own. */
    fun allEngines(): List<SearchEngine> = SearchEngine.all + customEngines

    /** The engine an id names, among all of them, or the standard one. */
    fun engineNamed(id: String?): SearchEngine =
        allEngines().firstOrNull { it.id == id } ?: SearchEngine.standard

    /** Whether the UI is dark now: the app's own setting when it says light or dark, the
     * system's when it follows it. A tab with no content choice of its own goes the same way. */
    fun resolvedDark(systemDark: Boolean): Boolean = when (darkMode) {
        "light" -> false
        "dark" -> true
        else -> systemDark
    }

    /** "system", "light" or "dark": null follows the system. */
    var darkMode: String
        get() = prefs.getString("darkMode", "system") ?: "system"
        set(value) {
            prefs.edit().putString("darkMode", value).apply()
            darkModeFlow.value = value
        }

    /** What pages do when the app is dark: "follow" (the default) keeps each page's own
     * dark where it has one, "darken" also darkens the ones without, "off" never changes
     * a page's colors. The app's own look is darkMode's business, as it was. */
    var pageDark: String
        get() = when (prefs.getString("pageDark", null)) {
            "darken", "off" -> prefs.getString("pageDark", null)!!
            else -> "follow"
        }
        set(value) { prefs.edit().putString("pageDark", value).apply() }

    /** What an up swipe in the switcher does: "close" (the default) or "archive". */
    var swipeUpAction: String
        get() = prefs.getString("swipeUpAction", "close") ?: "close"
        set(value) { prefs.edit().putString("swipeUpAction", value).apply() }

    /** Whether the down swipe archives: off makes it do nothing, so the user can keep the card
     * still while flipping. */
    var swipeDownArchives: Boolean
        get() = prefs.getBoolean("swipeDownArchives", true)
        set(value) { prefs.edit().putBoolean("swipeDownArchives", value).apply() }

    /** What the down swipe does: "archive" (the default) or "none". Kept beside the old
     * boolean for a session saved by an older build. */
    var swipeDownAction: String
        get() = when (prefs.getString("swipeDownAction", null)) {
            "close", "archive", "none" -> prefs.getString("swipeDownAction", null)!!
            else -> if (swipeDownArchives) "archive" else "none"
        }
        set(value) {
            prefs.edit().putString("swipeDownAction", value).apply()
            swipeDownArchives = value != "none"
        }

    /** What a Space with no tabs shows: "panel" (the new page panel, the default), "last" (the
     * most recent page from history), "page" (a chosen address), "search" or "blank", as the
     * owner asked for a start of the user's own choice. */
    var startPage: String
        get() {
            val stored = prefs.getString("startPage", null)
            return when (stored) {
                "last", "page", "search", "blank" -> stored
                // Unset or unknown starts at the panel, the owner's asked-for default.
                else -> "panel"
            }
        }
        set(value) { prefs.edit().putString("startPage", value).apply() }

    /** The address a "page" start page opens. */
    var startPageUrl: String
        get() = prefs.getString("startPageUrl", null) ?: "https://ilobrowser.com"
        set(value) { prefs.edit().putString("startPageUrl", value).apply() }

    /** How many recent pages the new page panel shows as tiles, 0 hiding them. */
    var recentTiles: Int
        get() = prefs.getInt("recentTiles", 8).coerceIn(0, 16)
        set(value) { prefs.edit().putInt("recentTiles", value.coerceIn(0, 16)).apply() }

    /** How long the undo notice of a thrown card waits, in seconds, before its move stands. */
    var undoSeconds: Int
        get() = prefs.getInt("undoSeconds", 5).coerceIn(1, 30)
        set(value) { prefs.edit().putInt("undoSeconds", value.coerceIn(1, 30)).apply() }

    /** The switcher card's width as a share of the deck's, the owner's number the default;
     * the deck fits its fan to the screen at any value, so the whole range is open. */
    var switcherCardShare: Float
        get() = prefs.getFloat("switcherCardShare", 0.75f).coerceIn(0.10f, 0.95f)
        set(value) { prefs.edit().putFloat("switcherCardShare", value.coerceIn(0.10f, 0.95f)).apply() }

    /** How much of a behind card the one in front covers, the owner's number the default;
     * the step the cards stand apart is the share left showing. */
    var switcherOverlap: Float
        get() = prefs.getFloat("switcherOverlap", 0.70f).coerceIn(0.10f, 0.95f)
        set(value) { prefs.edit().putFloat("switcherOverlap", value.coerceIn(0.10f, 0.95f)).apply() }

    /** The language a page is put into, as the owner asked the translation to follow the
     * setting: a BCP-47 code, or null for the phone's own language. */
    var translationLanguage: String?
        get() = prefs.getString("translationLanguage", null)
        set(value) { prefs.edit().putString("translationLanguage", value).apply() }

    /** Off keeps today's behavior: nothing leaves for another app until the user asks. */
    var openInApps: Boolean
        get() = prefs.getBoolean("openInApps", false)
        set(value) { prefs.edit().putBoolean("openInApps", value).apply() }

    /** How far a card drag must go to commit: the owner's reach choice, half the screen
     * the Arc-like default. */
    var swipeReach: Float
        get() = prefs.getFloat("swipeReach", 0.5f).coerceIn(0.25f, 1f)
        set(value) { prefs.edit().putFloat("swipeReach", value.coerceIn(0.25f, 1f)).apply() }

    init { darkModeFlow.value = darkMode }
}

/**
 * The browser in one object: tab state, the page engine and what the user chose. The UI holds
 * one of these; the sync lane reaches in through [accountHook] and [BrowserState.spacesChanged].
 */
class Browser(context: Context, val blocker: EasyListBlocker) : PageListener {
    val state = BrowserState(context.filesDir)
    val settings = BrowserSettings(context.getSharedPreferences("ilo", Context.MODE_PRIVATE))
    val pictures = TabPictures(File(context.cacheDir, "tabs"))
    /** The icons sites gave, learned as their pages open, for engine marks and recent tiles. */
    val favicons = FaviconStore(File(context.cacheDir, "favicons"))
    /** What the phone remembers for itself: pages it was on, bookmarks, downloads, the archive. */
    val stores = PersonalStores(context.filesDir)

    /** The answers the user gave a site's permission asks, kept per host across loads. */
    val sitePermissions = SitePermissionStore(context)

    /** The context the OS's own permission ask needs; kept because the ask outlives the page
     * that started it. */
    private val appContext = context.applicationContext
    /** The page asks waiting on the OS's own ask, by the code the bridge gave each. */
    private val pendingOs = java.util.concurrent.ConcurrentHashMap<Int, Pair<Tab, PendingAsk>>()
    private val osRequestCode = java.util.concurrent.atomic.AtomicInteger(1000)

    // A removed list refetches the rest in the background, so the screen never waits on it.
    private val blockingScope = kotlinx.coroutines.CoroutineScope(
        kotlinx.coroutines.Dispatchers.Default + kotlinx.coroutines.SupervisorJob()
    )

    val blocking = BlockingSettings(blocker, blockingScope)

    val engine = PageEngine(state, blocker, this)

    init {
        // The engine writes what the phone remembers: history from its pages, downloads from the
        // pages' own asks, both through the stores.
        engine.stores = stores
        engine.downloadStarter = { url, name ->
            val request = android.app.DownloadManager.Request(android.net.Uri.parse(url))
                .setNotificationVisibility(android.app.DownloadManager.Request.VISIBILITY_VISIBLE_NOTIFY_COMPLETED)
                .setDestinationInExternalPublicDir(
                    android.os.Environment.DIRECTORY_DOWNLOADS, name,
                )
            val manager = context.getSystemService(Context.DOWNLOAD_SERVICE) as android.app.DownloadManager
            manager.enqueue(request)
        }
        // Stored custom lists re-enter the engine at every launch, and a stale built-in fetches
        // again; neither is launch-blocking.
        blockingScope.launch {
            blocker.refreshCustomLists()
            blocker.updateIfStale()
        }
        // The engine asks the user through here when a page's permission ask has no stored
        // answer; the answer writes the store and answers the page's request itself.
        engine.sitePermissions = sitePermissions
        // The router reads the setting at click time, so a toggle applies to the next link.
        engine.openInApps = { settings.openInApps }
        // An Allow the OS no longer backs must not fail silently, so the engine asks here
        // before granting one; a no denies and clears the stale exception.
        engine.osGrantCheck = { kind ->
            val osKinds = osPermissionOf(kind)
            osKinds.isEmpty() || osKinds.all { ContextCompat.checkSelfPermission(appContext, it) == PackageManager.PERMISSION_GRANTED }
        }
        engine.permissionAsker = { tab, host, kind, request ->
            // A second ask in the same tab answers the first with no, so no page waits forever.
            state.tabAsks.value[tab.id]?.deny()
            state.tabAsks.update { it + (tab.id to PendingAsk(host, kind, { request.grant(request.resources) }, request::deny)) }
        }
        // The engine hands a tab's WebView over when the tab is frozen; the ask standing over
        // its page is answered with no first, so the WebView never dies on a waiting request.
        state.freeze = { tab ->
            state.tabAsks.value[tab.id]?.deny()
            state.tabAsks.update { it - tab.id }
            engine.freeze(tab)
        }
        // Pictures of tabs the last session no longer has go with it.
        pictures.prune(state.tabs.value.map { it.id })
    }

    /** The sync lane sets this to reach its account; the passwords lane's account arrives
     * with it, read from the account's own facade, so the UI and the lane never touch the
     * account itself. */
    var accountHook: Any? = null
        set(value) {
            field = value
            val account = value as? app.ilo.browser.android.sync.SyncedAccount
            passwords.setSite(account?.let(::passwordSite))
            bookmarksLane.setAccount(account?.let(::bookmarkSite))
        }

    /** The account's passwords as the lane asks for them, so a plain unit test stands in. */
    private fun passwordSite(account: app.ilo.browser.android.sync.SyncedAccount) =
        object : PasswordsLane.PasswordsSite {
            private val facade = account.passwords
            override fun logins(page: String) = facade.logins(page)
            override fun password(login: app.ilo.browser.android.sync.SavedLogin) = facade.password(login)
            override fun saving(username: String, password: String, page: String) =
                facade.saving(username, password, page)
            override fun save(username: String, password: String, page: String) =
                facade.save(username, password, page)
        }

    /** The passwords lane over the account the sync lane sets; private tabs and no-account
     * days get no offers, as the lane answers null for each ask. The engine hears the pages'
     * sign-in forms through it. */
    val passwords = PasswordsLane(context, null)

    /** The bookmarks lane over the same account: local stars reach it, its list becomes the
     * local one, and without it the local JSON keeps working untouched. */
    val bookmarksLane = BookmarksLane(stores, null)

    /** The account's bookmarks as the lane asks for them, so a plain unit test stands in. */
    private fun bookmarkSite(account: app.ilo.browser.android.sync.SyncedAccount) =
        object : BookmarksLane.Site {
            private val facade = account.bookmarks
            override val ready get() = account.bookmarksReady()
            override fun kept() = facade.kept().map {
                // Sync's time is microseconds since 1601; the store's own is unix milliseconds.
                BookmarksLane.Kept(it.url, it.title, (it.added - 11_644_473_600_000_000L) / 1000)
            }
            override fun add(url: String, title: String) = facade.add(url, title)
            override fun remove(url: String) = facade.remove(url)
            override val tick = account.bookmarksTick
        }

    /** Asks the phone's owner to confirm a fill with the phone's lock, as iOS's does: the
     * host, which owns the screen, says whether they did before `then` runs; null, or a
     * false answer, fills nothing. Set by the host when the lane was made. */
    var fillConfirmer: ((site: String, then: () -> Unit) -> Unit)? = null

    /** The selected tab's load progress, 0 to 100, or 100 once its page has settled. A tab
     * switch adopts the new tab's own word, so the bar says what the tab on screen is doing. */
    val progressFlow = kotlinx.coroutines.flow.MutableStateFlow(0)

    /** Each tab's load, so a switch back shows the tab's own word instead of the last one's. */
    private val loads = java.util.concurrent.ConcurrentHashMap<String, Int>()
    /** The tabs whose page is still on its way: WebView's 100 is held back until it finishes.
     * Declared before init, whose collectors read them as soon as they run. */
    private val loading = java.util.Collections.newSetFromMap(
        java.util.concurrent.ConcurrentHashMap<String, Boolean>())

    init {
        engine.passwords = passwords
        // A tab gone takes its login story with it, wherever the close came from: the switcher,
        // a card's close, the archive, all funnel through the state's own list.
        kotlinx.coroutines.CoroutineScope(kotlinx.coroutines.Dispatchers.Default + kotlinx.coroutines.SupervisorJob())
            .launch { state.tabs.collect { tabs ->
                val live = tabs.map { it.id }.toSet()
                passwords.ids().forEach { if (it !in live) passwords.close(it) }
                // A gone tab's load word goes with it, from every way a tab can go.
                (loading.toList() + loads.keys).forEach { if (it !in live) loadThrown(it) }
            } }
        // A switch takes the new tab's own load word, so the bar never carries the old one's.
        kotlinx.coroutines.CoroutineScope(kotlinx.coroutines.Dispatchers.Default + kotlinx.coroutines.SupervisorJob())
            .launch { state.selectedId.collect { id ->
                if (id != null) progressFlow.value = loads[id] ?: 0
            } }
    }

    override fun loaded(tabId: String, url: String, title: String) {
        state.tabs.value.firstOrNull { it.id == tabId }?.let { tab ->
            state.update(tab.copy(url = url, title = title))
        }
    }

    override fun progress(tabId: String, value: Int) {
        // WebView says 100 before subframes are done, so the load's own word ends it; while
        // it goes on, the line creeps and never says it's over.
        if (tabId in loading && value >= 100) return
        loads[tabId] = value
        state.tabs.value.firstOrNull { it.id == tabId }?.let { tab ->
            if (tab.id == state.selectedId.value) progressFlow.value = value
        }
    }

    override fun loadStarted(tabId: String) {
        loading.add(tabId)
        loads[tabId] = 1
        state.tabs.value.firstOrNull { it.id == tabId }?.let { tab ->
            if (tab.id == state.selectedId.value) progressFlow.value = 1
        }
    }

    override fun loadFinished(tabId: String) {
        loading.remove(tabId)
        loads[tabId] = 100
        state.tabs.value.firstOrNull { it.id == tabId }?.let { tab ->
            if (tab.id == state.selectedId.value) progressFlow.value = 100
        }
    }

    override fun favicon(tabId: String, color: Long) {
        state.tabs.value.firstOrNull { it.id == tabId }?.let { tab ->
            state.update(tab.copy(faviconColor = color))
        }
    }

    override fun pageIcon(tabId: String, url: String, icon: android.graphics.Bitmap) {
        // A page's icon is learned for its host only when the page has one to name, so an
        // about:blank start keeps whatever the host last gave.
        if (url.startsWith("http")) favicons.keep(icon, of = url)
    }

    override fun pageReady(tabId: String, url: String) {
        // The page is drawn: its picture is taken now, so the deck always has a fresh one even
        // if the tab is closed before it is ever left.
        state.tabs.value.firstOrNull { it.id == tabId }?.let { keepPicture(it) }
    }

    override fun themeColor(tabId: String, color: Long?) {
        // The page named no color only on its first page, so the tab keeps one from a
        // earlier page until the next load answers for the page it is on.
        if (color != null) {
            state.tabs.value.firstOrNull { it.id == tabId }?.let { tab ->
                if (tab.pageColor != color) state.update(tab.copy(pageColor = color))
            }
        }
    }

    override fun newWindow(openerId: String, webView: android.webkit.WebView) {}

    /** The link a long press holds, shown in the sheet with its two ways to open elsewhere;
     * null when no sheet is up. */
    val pressedLink = kotlinx.coroutines.flow.MutableStateFlow<String?>(null)

    /** An external app a page asked for, waiting on the user's word; null when no ask is up. */
    val externalAsk =
        kotlinx.coroutines.flow.MutableStateFlow<PendingExternal?>(null)

    /** The page's ask handed to the sheet: what to launch, the name the system resolved
     * (null keeps the router's guess), the guess, the app's own icon when it resolved one,
     * and where to fall back on no. */
    fun askExternal(url: String, appName: String?, guess: String, icon: android.graphics.drawable.Drawable?, fallbackUrl: String?) {
        externalAsk.value = PendingExternal(url, appName, guess, icon, fallbackUrl)
    }

    /** The ask's answer: yes hands the system the URI, and a yes nothing answers lands on
     * the fallback page; no stays on the page, as the button says. */
    fun answerExternal(open: Boolean) {
        val ask = externalAsk.value ?: return
        externalAsk.value = null
        if (!open) return
        val intent = runCatching {
            android.content.Intent.parseUri(ask.url, android.content.Intent.URI_INTENT_SCHEME)
        }.getOrNull()
        if (intent == null || intent.resolveActivity(appContext.packageManager) == null) {
            // A yes nothing answers must not look like a broken tap: say it, then go back
            // only where the link itself named a place.
            android.widget.Toast.makeText(
                appContext,
                appContext.getString(R.string.open_in_app_not_found),
                android.widget.Toast.LENGTH_SHORT,
            ).show()
            ask.fallbackUrl?.let { go(it) }
            return
        }
        runCatching { appContext.startActivity(intent.addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK)) }
    }

    /** The ask's way out without opening anything. */
    fun dismissExternal() {
        externalAsk.value = null
    }

    override fun linkLongPressed(tabId: String, url: String) {
        pressedLink.value = url
    }

    override fun externalLinkAsked(tabId: String, url: String, appName: String?, fallbackUrl: String?) {
        // A second ask replaces the first, as the permission ask does, so no page waits.
        val guess = appName ?: ""
        val resolved = runCatching {
            android.content.Intent.parseUri(url, android.content.Intent.URI_INTENT_SCHEME)
                .resolveActivityInfo(appContext.packageManager, 0)
        }.getOrNull()
        val label = resolved?.loadLabel(appContext.packageManager)?.toString()
        val icon = resolved?.let { runCatching { it.loadIcon(appContext.packageManager) }.getOrNull() }
        askExternal(url, label, guess, icon, fallbackUrl)
    }

    /** The sheet's two ways out. Open in a new tab keeps the current mode; a private tab is
     * always its own ask, never the tab's own state, so an ordinary page's link opens a
     * private tab as the sheet says. */
    fun openPressedLink(private: Boolean) {
        val url = pressedLink.value ?: return
        pressedLink.value = null
        openInNewTab(url, isPrivate = private)
    }

    /** The sheet's way out without opening anything. */
    fun dismissPressedLink() {
        pressedLink.value = null
    }

    /** What the sheet asks about: where to go, the name the system resolved (null keeps the
     * router's guess), the guess itself, the app's own icon when it resolved one, and the
     * page to fall back to when the answer is no or nothing answers. */
    data class PendingExternal(
        val url: String,
        val appName: String?,
        val guess: String,
        val icon: android.graphics.drawable.Drawable?,
        val fallbackUrl: String?,
    )

    /** What the field opens: the address `text` names, or a search for it. */
    fun resolve(text: String): String? = Address.resolve(text, settings.searchEngine)

    /** Where a typed field sends its words: the page that's up when one is, the way iOS's go
     * takes a search made over a page, and a new tab otherwise. */
    fun go(text: String) {
        val url = resolve(text) ?: return
        val tab = state.selected
        if (tab != null) {
            // A new address ends any translation, so the way back goes with it.
            if (tab.originalUrl != null) state.update(tab.copy(originalUrl = null))
            engine.load(tab.id, url)
        } else openInNewTab(url)
    }

    fun openInNewTab(text: String, isPrivate: Boolean = state.privateMode.value) {
        val url = resolve(text) ?: return
        openUrlInNewTab(url, isPrivate)
    }

    /** A URL, not typed text: opened as it is, never a search. A tab from another computer
     * carries whatever URL it holds there, data: included. */
    fun openUrlInNewTab(url: String, isPrivate: Boolean = state.privateMode.value) {
        // An address the Space holds already is shown there, not opened twice, as iOS's openNew does.
        // In private mode the address goes to a private tab, as no ordinary tab is looked up then.
        if (!isPrivate) state.tabAt(url)?.let { state.select(it.id); return }
        val tab = state.newTab(url = url, isPrivate = isPrivate)
        if (isPrivate) engine.load(tab.id, url)
    }

    /** A private tab with nothing in it, shown over the Space: the field says where it goes.
     * The mode is set, so tabs opened after it are private too, as iOS's private browsing holds. */
    fun openPrivateTab() {
        state.privateMode.value = true
        state.newTab(isPrivate = true)
    }

    /** A tab goes to the archive instead of closing: its record kept, its page closed. */
    fun archiveTab(tab: Tab) {
        stores.archive(tab)
        engine.destroy(tab)
        loadThrown(tab.id)
        state.close(tab.id)
    }

    /** A thrown tab's load word goes with it, so a closed tab's state never lingers. */
    private fun loadThrown(tabId: String) {
        loading.remove(tabId)
        loads.remove(tabId)
    }

    /** An archived tab comes back as a tab, where it was; nothing of it stays in the archive. */
    fun restoreArchived(id: String) {
        val archived = stores.unarchive(id) ?: return
        val tab = state.newTab(url = archived.url, space = archived.space, isPrivate = false)
        state.update(tab.copy(title = archived.title, faviconColor = archived.faviconColor, wantsDesktop = archived.wantsDesktop, contentDark = archived.contentDark))
    }

    /** The last thrown card, its move, and the moment its move stands: what the undo notice
     * shows. Undo brings the tab back where it was, opened, before its time runs out. */
    class Thrown(
        val kind: String,
        val tab: Tab,
        val at: Long,
        val undo: () -> Unit,
    )

    /** The card the user threw and can still take back, or null when the last throw stood. */
    val thrownFlow = kotlinx.coroutines.flow.MutableStateFlow<Thrown?>(null)

    private var thrownJob: kotlinx.coroutines.Job? = null
    private val thrownScope = kotlinx.coroutines.MainScope()

    /** A card thrown in the switcher: its tab moves now, and undo brings it back while the
     * notice's countdown runs. A second throw replaces the first, whose move stands. */
    fun thrown(kind: String, tab: Tab, move: () -> Unit) {
        thrownJob?.cancel()
        move()
        thrownFlow.value = Thrown(kind, tab, System.currentTimeMillis()) {
            when (kind) {
                "closed" -> {
                    val restored = state.newTab(url = tab.url, isPrivate = tab.isPrivate, space = tab.space)
                    state.update(restored.copy(title = tab.title, faviconColor = tab.faviconColor, wantsDesktop = tab.wantsDesktop, contentDark = tab.contentDark))
                }
                "archived" -> restoreArchived(tab.id)
            }
        }
        thrownJob = thrownScope.launch {
            kotlinx.coroutines.delay(settings.undoSeconds * 1000L)
            thrownFlow.value = null
        }
    }

    /** The thrown card's move stands: the notice goes without bringing the tab back. */
    fun thrownStands() {
        thrownJob?.cancel()
        thrownFlow.value = null
    }

    /** Undo brings the tab back before its time runs out; the notice goes with it. */
    fun undoThrown() {
        val thrown = thrownFlow.value ?: return
        thrownJob?.cancel()
        thrownFlow.value = null
        thrown.undo()
    }

    /** The page through Google's translation service, the service set to the page's own address
     * and the address kept on the tab as the way back, as the owner chose for a WebView with
     * no translator of its own. */
    fun translate(tab: Tab, target: String) {
        val url = tab.originalUrl ?: tab.url ?: return
        val page = java.net.URLEncoder.encode(url, "UTF-8")
        state.update(tab.copy(originalUrl = url))
        engine.load(tab.id, "https://translate.google.com/translate?sl=auto&tl=$target&u=$page")
    }

    /** The page's own words back, the translation's address dropped from the tab. */
    fun showOriginal(tab: Tab) {
        val url = tab.originalUrl ?: return
        state.update(tab.copy(originalUrl = null))
        engine.load(tab.id, url)
    }

    /** A tab's page is left: its picture goes to the cache before its WebView can go, so its card
     * holds what it last showed, as iOS's takePicture runs in leave(). Never for a private tab:
     * its page is never pictured, not in memory and not in a file. */
    fun keepPicture(tab: Tab) {
        if (tab.isPrivate) return
        engine.picture(tab)?.let { pictures.keep(it, of = tab.id) }
    }

    /** What a page's own dark is when its tab hasn't picked one: the app's general setting
     * resolved against the system's dark each time a page is drawn, and only when the pages
     * setting asks for darkening at all; "follow" leaves each page to its own dark mode,
     * which the WebView offers through the theme without any darkening engine. */
    fun contentDarkDefault(systemDark: Boolean): Boolean =
        settings.resolvedDark(systemDark) && settings.pageDark == "darken"

    fun loadInSelected(url: String) {
        val tab = state.selected ?: state.newTab(url)
        engine.load(tab.id, url)
    }

    /** Answer the certificate warning standing in `tab`'s page's place: Allow trusts exactly
     * that certificate at that address, Go back turns the page away, and one stood back up by
     * the page itself (WebView asks nothing again of a server it holds) is answered with no
     * handler: Allow trusts and loads again, Go back goes to the page before or out to the Space. */
    fun answerSsl(tab: Tab, fault: CertificateFault, allow: Boolean) {
        val pending = engine.pendingSsl.remove(tab.id)
        if (pending != null && pending.second == fault) {
            if (allow) {
                state.exceptionsFor(tab).trust(fault.exception)
                // The row that takes the trust back lists while a page that came in by it is up.
                state.tabExceptions.update { it + (tab.id to fault.exception) }
                pending.first.proceed()
            } else {
                engine.turnedAway[tab.id] = "${fault.exception.host}:${fault.exception.port}"
                pending.first.cancel()
            }
        } else {
            // No handler answers this one, and one left from an older load goes with it.
            pending?.let { it.first.cancel() }
            if (allow) {
                state.exceptionsFor(tab).trust(fault.exception)
                state.tabExceptions.update { it + (tab.id to fault.exception) }
                engine.reload(tab)
            } else {
                engine.turnedAway[tab.id] = "${fault.exception.host}:${fault.exception.port}"
                if (engine.canGoBack(tab)) engine.goBack(tab) else state.leaveToSpace()
            }
        }
        state.tabFaults.value = state.tabFaults.value - tab.id
    }

    /** Takes back Visit Anyway for the page on screen and loads it again: the warning stands in
     * its place, whether WebView asks the server for its certificate again or not. */
    fun stopTrusting(tab: Tab) {
        val exception = state.tabExceptions.value[tab.id] ?: return
        state.exceptionsFor(tab).stop(exception)
        state.tabExceptions.update { it - tab.id }
        engine.reload(tab)
    }

    /** Answer the permission ask standing over `tab`'s page: the answer writes the site's own
     * store, answers the page's request, and the ask leaves with the answer. Allow first asks
     * the OS for what the app itself must hold, as WebView grants a page only that. */
    fun answerSite(tab: Tab, ask: PendingAsk, allow: Boolean) {
        if (!allow) {
            sitePermissions.setAnswer(ask.host, ask.kind, SiteAnswer.BLOCK)
            ask.deny()
            state.tabAsks.update { it - tab.id }
            return
        }
        sitePermissions.setAnswer(ask.host, ask.kind, SiteAnswer.ALLOW)
        val osKinds = osPermissionOf(ask.kind)
        if (osKinds.isEmpty() || osKinds.all { ContextCompat.checkSelfPermission(appContext, it) == PackageManager.PERMISSION_GRANTED }) {
            ask.grant()
            state.tabAsks.update { it - tab.id }
            return
        }
        // The system's own ask decides here, and its answer answers the page's request.
        val code = osRequestCode.incrementAndGet()
        pendingOs[code] = tab to ask
        val bridge = osPermissionBridge
        if (bridge == null) {
            pendingOs.remove(code)
            ask.deny()
            state.tabAsks.update { it - tab.id }
        } else {
            bridge(osKinds, code)
        }
    }

    /** What the OS calls the kind a page asked for, or nothing where the OS asks nothing,
     * as notifications before API 33 are the app's own to give. */
    private fun osPermissionOf(kind: SitePermission): Array<String> = when (kind) {
        SitePermission.LOCATION -> arrayOf(
            Manifest.permission.ACCESS_FINE_LOCATION, Manifest.permission.ACCESS_COARSE_LOCATION,
        )
        SitePermission.CAMERA -> arrayOf(Manifest.permission.CAMERA)
        SitePermission.MICROPHONE -> arrayOf(Manifest.permission.RECORD_AUDIO)
        SitePermission.NOTIFICATIONS ->
            if (android.os.Build.VERSION.SDK_INT >= 33) arrayOf(Manifest.permission.POST_NOTIFICATIONS) else emptyArray()
    }

    /** The activity's own way to put up the system's permission ask; the result comes back
     * through [onOsPermissionResult], which answers the page's request waiting behind it. */
    var osPermissionBridge: ((permissions: Array<String>, requestCode: Int) -> Unit)? = null

    /** The OS ask's answer to the page's request waiting behind it: a no there is a no for
     * the site too, as Chrome's own dialog takes the site's answer with it. */
    fun onOsPermissionResult(requestCode: Int, granted: Boolean) {
        val pending = pendingOs.remove(requestCode) ?: return
        val (tab, ask) = pending
        // The WebView may have gone while the system's dialog stood; its request answers nothing.
        if (granted) {
            runCatching { ask.grant() }
        } else {
            sitePermissions.setAnswer(ask.host, ask.kind, SiteAnswer.BLOCK)
            runCatching { ask.deny() }
        }
        state.tabAsks.update { it - tab.id }
    }
}
