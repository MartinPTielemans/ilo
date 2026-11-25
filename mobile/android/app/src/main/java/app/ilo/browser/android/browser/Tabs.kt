package app.ilo.browser.android.browser

import android.os.Bundle
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.io.File
import java.util.ArrayDeque

/** A tab: what the UI needs to show it. The WebView's saved state rides along when a tab is frozen. */
data class Tab(
    val id: String,
    val url: String? = null,
    val title: String = "",
    val faviconColor: Long = 0x5C38,
    val isPrivate: Boolean = false,
    val space: String = Space.DEFAULT,
    /** Whether the page is asked for as a desktop's, as iOS's wantsDesktopSite. */
    val wantsDesktop: Boolean = false,
    /** The page's text zoom in percent, as Chrome's page zoom is the WebView's own text zoom. */
    val pageZoom: Int = 100,
    /** The page's own address when translate.goog holds its translation, the way back. */
    val originalUrl: String? = null,
    /** The page's own dark, one tab at a time, as the owner asked: a new tab follows the app's
     * general dark setting, and the tools menu turns it for the tab alone. */
    val contentDark: Boolean? = null,
    /** The color the page's own theme-color meta named, as Chrome's bar takes, or null when it
     * named none and the bar keeps the Space's tint. */
    val pageColor: Long? = null,
    val webViewStateBundle: Bundle? = null,
)

/** A Space is a tab group: tabs belong to one, the strip switches between them. */
data class Space(
    val id: String,
    val name: String,
    /** The Space's colour as 0xRRGGBB, or null when it has none, as the iOS SyncedSpace's does. */
    val color: Long? = null,
) {
    companion object { const val DEFAULT = "space-default" }
}

/** The part of a tab that survives a relaunch: private tabs never persist. */
@Serializable
private data class TabRecord(
    val id: String,
    val url: String?,
    val title: String,
    val space: String,
    val faviconColor: Long,
    val wantsDesktop: Boolean = false,
    val pageZoom: Int = 100,
    val originalUrl: String? = null,
    /** Null follows the app's general dark setting, as a tab the menu never touched does. */
    val contentDark: Boolean? = null,
    /** The page's theme-color, kept so the bar holds it through a relaunch. */
    val pageColor: Long? = null,
)

@Serializable
private data class Session(
    val spaces: List<SpaceRecord>,
    val currentSpace: String,
    val selected: String?,
    val tabs: List<TabRecord>,
)

@Serializable
private data class SpaceRecord(val id: String, val name: String, val color: Long?)

/**
 * Which tabs are open, which Space is up, and which tab is on screen. The UI recomposes on these
 * flows alone; nothing here polls.
 */
class BrowserState(private val filesDir: File) {
    val tabs = MutableStateFlow<List<Tab>>(emptyList())
    val selectedId = MutableStateFlow<String?>(null)
    /** The page last on screen, whose card the Space rings: where the user came from. */
    val lastShownId = MutableStateFlow<String?>(null)
    val spaces = MutableStateFlow<List<Space>>(defaultSpaces())
    val currentSpace = MutableStateFlow(Space.DEFAULT)
    val blockedCount = MutableStateFlow(0)
    /** The matches the page's own find bar last counted, for the tab it ran on. */
    val findCount = MutableStateFlow(0)
    /** The last session write that failed, or null when the session was written or never tried. */
    val persistError = MutableStateFlow<String?>(null)
    /** The certificate warning standing in a tab's page's place, keyed by tab id: a tab's
     * warning is its own, as iOS keeps the failure on the tab. */
    val tabFaults = MutableStateFlow<Map<String, CertificateFault>>(emptyMap())
    /** A permission ask waiting for the user's answer, keyed by tab id: the ask stands over
     * the page until it is answered, and only one stands per tab. */
    val tabAsks = MutableStateFlow<Map<String, PendingAsk>>(emptyMap())
    /** Each tab's trusted certificate exception, the way back from Visit Anyway, keyed by tab id.
     * Kept per tab because a private tab's exceptions are its own and die with it. */
    val tabExceptions = MutableStateFlow<Map<String, CertificateException>>(emptyMap())
    /** Whether the page's bar is tucked to its small pill: the page's scroll asks for it. */
    val barTucked = MutableStateFlow(false)
    /** Whether the selected tab's page sits at its very top, so a pull there is a refresh, as
     * Chrome's is. */
    val pageAtTop = MutableStateFlow(true)

    /** Certificates the user went on past, shared by the normal tabs. */
    val exceptions = CertificateExceptions()
    private val privateExceptions = HashMap<String, CertificateExceptions>()

    /** Freezing is decided by use order: the most recently shown tabs stay live. */
    private val lru = ArrayDeque<String>()
    var liveLimit = 4
    /** Called by the owner of the WebViews when a tab must give its WebView up. */
    var freeze: ((Tab) -> Unit)? = null

    /**
     * The sync lane sets this to hear Space changes and pushes account Spaces back through
     * [setSpaces]; no code here depends on the account.
     */
    var spacesChanged: ((List<Space>) -> Unit)? = null

    init { restore() }

    fun exceptionsFor(tab: Tab): CertificateExceptions =
        if (tab.isPrivate) privateExceptions.getOrPut(tab.id) { CertificateExceptions() } else exceptions

    val selected: Tab? get() = tabs.value.firstOrNull { it.id == selectedId.value }
    fun tabsIn(space: String = currentSpace.value): List<Tab> = tabs.value.filter { it.space == space }

    /** Whether what's on screen is a private tab's: its page, or its card in the Space. */
    fun showsPrivate(): Boolean =
        selected?.isPrivate == true || tabsIn().any { it.isPrivate }

    /** The mode the + modal's private row set: new tabs come up private until a normal tab is
     * opened, as iOS's private browsing mode holds while it's on. */
    val privateMode = MutableStateFlow(false)

    /** The ordinary tab in this Space at `url`: what the field shows instead of opening twice,
     * as iOS's openNew finds the tab the address names. A private tab is never it. */
    fun tabAt(url: String): Tab? = tabsIn().firstOrNull { !it.isPrivate && it.url == url }

    fun touch(id: String) {
        lru.remove(id)
        lru.addFirst(id)
        freezeBeyondLimit()
    }

    private fun freezeBeyondLimit() {
        while (lru.size > liveLimit) {
            val id = lru.removeLast()
            if (id == selectedId.value) { lru.addFirst(id); break }
            val tab = tabs.value.firstOrNull { it.id == id } ?: continue
            freeze?.invoke(tab)
        }
    }

    fun newTab(url: String? = null, isPrivate: Boolean = privateMode.value, space: String = currentSpace.value): Tab {
        val tab = Tab(id = newId(), url = url, isPrivate = isPrivate, space = space,
            faviconColor = if (isPrivate) 0x4B5563 else 0x5C38)
        tabs.value = tabs.value + tab
        select(tab.id)
        return tab
    }

    fun select(id: String) {
        val tab = tabs.value.firstOrNull { it.id == id } ?: return
        currentSpace.value = tab.space
        selectedId.value = id
        lastShownId.value = id
        touch(id)
        persist()
    }

    /** The page goes back to its Space: the tab stays open, its card ringed, nothing on screen. */
    fun leaveToSpace() {
        selectedId.value = null
        persist()
    }

    fun close(id: String) {
        val tab = tabs.value.firstOrNull { it.id == id } ?: return
        tabs.value = tabs.value.filterNot { it.id == id }
        lru.remove(id)
        privateExceptions.remove(id)
        tabExceptions.value = tabExceptions.value - id
        tabFaults.value = tabFaults.value - id
        // The ask standing over the closed tab is answered with no, so its page waits no more.
        tabAsks.value[id]?.deny()
        tabAsks.value = tabAsks.value - id
        if (lastShownId.value == id) lastShownId.value = null
        if (selectedId.value == id) {
            // The next tab up is the closed one's kind, so a private page closes into a private
            // page, as iOS keeps its kinds apart; none left and the Space shows itself.
            val next = tabsIn(tab.space).filter { it.isPrivate == tab.isPrivate }.lastOrNull()
            if (next != null) select(next.id) else selectedId.value = null
        }
        // The private mode ends when its last tab is gone, as iOS's private browsing leaves.
        if (privateMode.value && tabs.value.none { it.isPrivate }) privateMode.value = false
        persist()
    }

    fun update(tab: Tab) {
        tabs.value = tabs.value.map { if (it.id == tab.id) tab else it }
        persist()
    }

    fun selectSpace(id: String) {
        if (spaces.value.none { it.id == id }) return
        currentSpace.value = id
        // The Space's last-open tab of the mode's kind comes up; a Space without one shows itself.
        val kind = privateMode.value
        val ofKind = tabsIn(id).filter { it.isPrivate == kind }
        if (ofKind.isNotEmpty()) select(ofKind.last().id) else selectedId.value = null
        spacesChanged?.invoke(spaces.value)
    }

    fun addSpace(name: String): Space {
        val space = Space(newId(), name)
        spaces.value = spaces.value + space
        spacesChanged?.invoke(spaces.value)
        persist()
        return space
    }

    /** For the sync lane: replace the Space list, keeping the current one when it survives. */
    fun setSpaces(list: List<Space>) {
        spaces.value = if (list.isEmpty()) defaultSpaces() else list
        if (spaces.value.none { it.id == currentSpace.value }) currentSpace.value = spaces.value.first().id
        spacesChanged?.invoke(spaces.value)
    }

    private fun defaultSpaces() = listOf(Space(Space.DEFAULT, "Personal"))

    private fun newId() = java.util.UUID.randomUUID().toString()

    private fun persist() {
        val session = Session(
            spaces = spaces.value.map { SpaceRecord(it.id, it.name, it.color) },
            currentSpace = currentSpace.value,
            selected = selectedId.value,
            tabs = tabs.value.filterNot { it.isPrivate }.map { TabRecord(it.id, it.url, it.title, it.space, it.faviconColor, it.wantsDesktop, it.pageZoom, it.originalUrl, it.contentDark, it.pageColor) },
        )
        // A private tab is never written anywhere, so it isn't in the session.
        // A failed write is surfaced rather than swallowed: a session that stops persisting
        // is a bug the UI must be able to show and a test can assert.
        persistError.value = try {
            File(filesDir, SESSION_FILE).writeText(json.encodeToString(session)); null
        } catch (e: Exception) {
            e.message
        }
    }

    private fun restore() {
        val file = File(filesDir, SESSION_FILE)
        if (!file.exists()) { if (spaces.value.isEmpty()) spaces.value = defaultSpaces(); return }
        val session = runCatching { json.decodeFromString<Session>(file.readText()) }.getOrNull() ?: return
        spaces.value = session.spaces.map { Space(it.id, it.name, it.color) }
            .ifEmpty { defaultSpaces() }
        currentSpace.value = if (spaces.value.any { it.id == session.currentSpace }) session.currentSpace else spaces.value.first().id
        tabs.value = session.tabs.map { Tab(it.id, it.url, it.title, faviconColor = it.faviconColor, space = it.space, wantsDesktop = it.wantsDesktop, pageZoom = it.pageZoom, originalUrl = it.originalUrl, contentDark = it.contentDark, pageColor = it.pageColor) }
        selectedId.value = tabs.value.firstOrNull { it.id == session.selected }?.id ?: tabs.value.lastOrNull()?.id
        tabs.value.forEach { lru.addLast(it.id) }
    }

    companion object {
        private const val SESSION_FILE = "session.json"
        private val json = Json { ignoreUnknownKeys = true }
    }
}

/** A search engine the field can send a search to. Nothing is asked of any server while typing. */
@kotlinx.serialization.Serializable
data class SearchEngine(val id: String, val name: String, val prefix: String) {
    fun search(query: String): String {
        // URLEncoder turns a space into +, which a search engine reads as a plus sign.
        val encoded = java.net.URLEncoder.encode(query, "UTF-8").replace("+", "%20")
        return prefix + encoded
    }

    /** The icon the app ships with for the engine, when its site hasn't given one of its own.
     * Read through a function, so the class init never touches the companion before its time. */
    val bundledIcon: Int? get() = iconIds[id]

    companion object {
        val all = listOf(
            SearchEngine("duckduckgo", "DuckDuckGo", "https://duckduckgo.com/?q="),
            SearchEngine("brave", "Brave Search", "https://search.brave.com/search?q="),
            SearchEngine("startpage", "Startpage", "https://www.startpage.com/do/search?q="),
            SearchEngine("ecosia", "Ecosia", "https://www.ecosia.org/search?q="),
            SearchEngine("kagi", "Kagi", "https://kagi.com/search?q="),
            SearchEngine("google", "Google", "https://www.google.com/search?q="),
            SearchEngine("bing", "Bing", "https://www.bing.com/search?q="),
        )
        val standard = all[0]
        fun named(id: String?): SearchEngine = all.firstOrNull { it.id == id } ?: standard

        // The engine ids the app ships an icon for, drawn from the drawable named after each:
        // declared after `all`, so it's read only once the class is up, never while it builds.
        private val iconIds = mapOf(
            "duckduckgo" to app.ilo.browser.android.R.drawable.engine_duckduckgo,
            "brave" to app.ilo.browser.android.R.drawable.engine_brave,
            "startpage" to app.ilo.browser.android.R.drawable.engine_startpage,
            "ecosia" to app.ilo.browser.android.R.drawable.engine_ecosia,
            "kagi" to app.ilo.browser.android.R.drawable.engine_kagi,
            "google" to app.ilo.browser.android.R.drawable.engine_google,
            "bing" to app.ilo.browser.android.R.drawable.engine_bing,
        )
    }
}

/** The address `text` names, or a search for it. Ports the iOS app's Address.swift. */
object Address {
    /** The URL to open, or null when the text is empty. */
    fun resolve(text: String, engine: SearchEngine): String? {
        val trimmed = text.trim()
        if (trimmed.isEmpty()) return null
        return asAddress(trimmed) ?: engine.search(trimmed)
    }

    /** `text` as a web address, if it reads as one: an http(s) URL, or a host with a dot,
     * localhost or an IP address. Another scheme is searched for rather than opened. */
    fun asAddress(text: String): String? {
        if (text.isEmpty() || text.any { it.isWhitespace() }) return null
        val lower = text.lowercase()
        if (lower.startsWith("http://") || lower.startsWith("https://")) {
            val uri = runCatching { java.net.URI(text) }.getOrNull() ?: return null
            return if (uri.host.isNullOrEmpty()) null else text
        }
        if (text.contains("://") || text.contains("@")) return null

        val authority = text.takeWhile { !"/?#".contains(it) }
        val host = authority.substringBefore(":").lowercase()
        val local = host == "localhost" || isIpv4(host)
        if (!local && !namesSite(host)) return null
        // A machine on the user's own network rarely has a certificate.
        return (if (local) "http://" else "https://") + text
    }

    private fun isIpv4(host: String): Boolean {
        val parts = host.split(".")
        return parts.size == 4 && parts.all { it.toIntOrNull()?.let { v -> v in 0..255 } == true }
    }

    /** Endings a word with a dot more often has as a file's name than as a site's. */
    private val fileEndings = setOf(
        "js", "ts", "py", "rs", "md", "txt", "json", "html", "css", "swift", "pdf", "png", "jpg", "zip",
        "toml", "yml", "yaml", "sh", "c", "h", "cpp", "go", "rb", "php", "exe",
    )

    private fun namesSite(host: String): Boolean {
        val labels = host.split(".")
        if (labels.size < 2) return false
        val last = labels.last()
        val labelsOk = labels.all { label ->
            label.isNotEmpty() && !label.startsWith("-") && !label.endsWith("-") &&
                label.all { it.isLetterOrDigit() || it == '-' }
        }
        return labelsOk && last.length >= 2 && last.all { it.isLetter() } && last !in fileEndings
    }
}