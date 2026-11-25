package app.ilo.browser.android.blocking

import android.content.Context
import android.content.SharedPreferences
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.net.HttpURLConnection
import java.net.URL

/**
 * Matching runs per network request on WebView's IO thread, so the hot path is plain
 * string work only: rules are precomputed into host-anchored parts, candidates come from
 * HashMap buckets keyed by the labels of the request's host plus one bucket of generic
 * rules, and no regex or allocation-heavy scan ever runs. Parsed lists live in one
 * immutable Engine swapped through a @Volatile field, so blocks() needs no lock.
 */
class EasyListBlocker(private val prefs: SharedPreferences) : Blocker {

    constructor(context: Context) : this(
        context.getSharedPreferences(PREFS_FILE, Context.MODE_PRIVATE)
    ) {
        this.context = context
        ensureLoaded()
    }

    private var context: Context? = null

    /** Immutable match state; swapped whole so readers never see a half-built index. */
    private class Engine(
        val byToken: Map<String, List<NetworkRule>>,
        /** Generic rules keyed by their longest plain run before '^'; a URL the rule matches
         * must contain that run verbatim, so a probe that misses it can skip the rule. */
        val byGoodToken: Map<String, List<NetworkRule>>,
        /** Generic rules with no plain run to key on, still scanned every request. */
        val untokened: List<NetworkRule>,
        /** The element-hiding state, rebuilt with the rules it runs on. */
        val hides: HideStyles.Built,
    )

    @Volatile
    private var engine = Engine(emptyMap(), emptyMap(), emptyList(), HideStyles.build(emptyList(), emptySet()))

    @Volatile
    private var siteExceptions: Set<String> = emptySet()

    private var builtIns: List<FilterList> = emptyList()
    private var customLists: List<FilterList> = emptyList()
    private var loadedBuiltIns = false

    /** uBlock's own CDN, at the commit the iPhone's Blocking/lists.ini pins its cookie lists
     * by; bump it there and here together. The two lists ilo added for the phone come from
     * their own sources, the ones uBlock's assets.json names for them, so they stay what
     * their publishers serve. */
    private val cdn =
        "https://raw.githubusercontent.com/uBlockOrigin/uAssetsCDN/e121e26b704ed001c4ff0b1efc74815259c684f2"

    /** The kinds of blocking, each with its own switch and the lists it runs on. The built-ins
     * come from ilo's own pins, the same sources the iPhone blocks with: uBlock's CDN for ads,
     * trackers and cookie notices, plus AdGuard's own mobile-ads list and uBlock's own fixes,
     * the two lists a phone's pages meet most. */
    enum class Kind(val label: String, val files: List<String>) {
        ADS("ads", listOf("easylist.txt", "ublock-filters.txt", "adguard-mobile.txt")),
        TRACKERS("trackers", listOf("easyprivacy.txt")),
        COOKIES("cookies", listOf("easylist-cookies.txt", "ublock-cookies.txt")),
    }

    /** Where each built-in list is fetched from when the user presses Update: uBlock's own
     * CDN at the commit the iPhone's Blocking/lists.ini pins its cookie lists by, so the two
     * block alike and move together; AdGuard's and uBlock's own lists from the addresses
     * uBlock's assets.json names for them. */
    val fetchUrls = mapOf(
        "easylist.txt" to "$cdn/thirdparties/easylist.txt",
        "easyprivacy.txt" to "$cdn/thirdparties/easyprivacy.txt",
        "easylist-cookies.txt" to "$cdn/thirdparties/easylist-cookies.txt",
        "ublock-cookies.txt" to "$cdn/filters/annoyances-cookies.txt",
        "ublock-filters.txt" to "https://ublockorigin.github.io/uAssets/filters/filters.txt",
        "adguard-mobile.txt" to "https://filters.adtidy.org/extension/ublock/filters/11.txt",
    )

    private val pageLock = Any()
    private var currentPage: String? = null
    private val pageBlockCount = java.util.concurrent.atomic.AtomicInteger()

    override var lastRule: String? = null
        private set

    /** Whether a kind's rules run, by its label; the Settings switch writes it. */
    fun kindOn(kind: Kind): Boolean = prefs.getBoolean("kind_${kind.label}", true)

    /** Whether pages hide the ad elements their lists name; the Settings switch writes it. */
    fun hidesOn(): Boolean = prefs.getBoolean(KEY_HIDES, true)

    /** Turns element hiding on or off and swaps the rebuilt engine in, as a kind does. */
    @Synchronized
    fun setHides(on: Boolean) {
        prefs.edit().putBoolean(KEY_HIDES, on).apply()
        rebuildEngine()
    }

    /** The stylesheet a page at `host` is told to hide with, or null when it hides nothing. */
    fun hidesFor(host: String?): HideStyles.Styles? {
        if (!isOn || !hidesOn()) return null
        val styles = HideStyles.forHost(engine.hides, host ?: return null)
        return if (styles.genericCss.isEmpty() && styles.hostCss.isEmpty()) null else styles
    }

    fun setKindOn(kind: Kind, on: Boolean) {
        prefs.edit().putBoolean("kind_${kind.label}", on).apply()
    }

    /** The lists a kind runs on, built-in names as the Settings screen shows them. */
    fun builtInListNames(kind: Kind): List<String> = kind.files

    /** The names of the lists the user added, the addresses as typed. */
    fun customListNames(): List<String> = customListUrls()

    /** Blocked requests on the page being viewed; a new page resets it. */
    val blockedCount: Int get() = pageBlockCount.get()

    override val isOn: Boolean
        get() = prefs.getBoolean(KEY_ON, true)

    fun setOn(on: Boolean) = prefs.edit().putBoolean(KEY_ON, on).apply()

    fun setSiteBlockingOff(origin: String, off: Boolean) {
        val host = hostOf(origin) ?: origin.lowercase()
        val next = siteExceptions.toMutableSet()
        if (off) next.add(host) else next.remove(host)
        prefs.edit().putStringSet(KEY_SITE_EXCEPTIONS, next).apply()
        siteExceptions = next
    }

    fun siteBlockingOff(origin: String): Boolean {
        val host = hostOf(origin) ?: origin.lowercase()
        return siteExceptions.any { domainMatches(host, it) }
    }

    /** The sites blocking is off for, by the name one owner registers. */
    fun siteExceptions(): List<String> = siteExceptions.sorted()

    override fun blocks(url: String, pageUrl: String?): Boolean {
        if (!isOn) return false
        val urlLower = url.lowercase()
        val host = hostOf(urlLower) ?: return false
        if (pageHostMatches(pageUrl)) return false
        return match(urlLower, host, cachedPageHost(pageUrl), pageUrl)
    }

    /** A tab's held choice runs over its site's: false keeps nothing from the page, true sets
     * the site's exception aside so the rules run even where its choice says off. */
    override fun blocks(url: String, pageUrl: String?, heldChoice: Boolean?): Boolean {
        if (heldChoice == false) return false
        if (heldChoice != true) return blocks(url, pageUrl)
        if (!isOn) return false
        val urlLower = url.lowercase()
        val host = hostOf(urlLower) ?: return false
        return match(urlLower, host, cachedPageHost(pageUrl), pageUrl)
    }

    /** The page's host, lowered once and kept for the page's many requests, since every one
     * of them carries the same pageUrl; a race here only repeats the same small work. */
    @Volatile
    private var pageHostCache: Pair<String, String?>? = null

    private fun cachedPageHost(pageUrl: String?): String? {
        if (pageUrl == null) return null
        val cached = pageHostCache
        if (cached?.first == pageUrl) return cached.second
        val host = hostOf(pageUrl.lowercase())
        pageHostCache = pageUrl to host
        return host
    }

    /** Whether the page's site has blocking off for it, as the site's own row in the sheet set. */
    private fun pageHostMatches(pageUrl: String?): Boolean {
        if (siteExceptions.isEmpty() || pageUrl == null) return false
        return siteExceptions.any { domainMatches(cachedPageHost(pageUrl) ?: return false, it) }
    }

    /** The rules' own answer, once the master switch and the site's choice have had theirs.
     * `url` and `host` arrive already lowercase, as the parser keeps its rules. */
    private fun match(url: String, host: String, pageHost: String?, pageUrl: String?): Boolean {
        if (pageUrl != null) {
            synchronized(pageLock) {
                if (currentPage != pageUrl) {
                    currentPage = pageUrl
                    pageBlockCount.set(0)
                }
            }
        }
        val rest = afterScheme(url, host)
        val e = engine
        // The earliest-parsed matching rule wins, so lastRule never depends on hash order.
        var blockingRule: NetworkRule? = null
        // The request's host is walked label by label, not split: a split would allocate a
        // list and every label up front, while a page's requests mostly match nothing.
        var at = 0
        while (true) {
            val dot = host.indexOf('.', at)
            val end = if (dot < 0) host.length else dot
            if (end > at) {
                val bucket = e.byToken[host.substring(at, end)]
                if (bucket != null) {
                    for (rule in bucket) {
                        if (!rule.matches(host, rest, pageHost)) continue
                        if (rule.exception) return false
                        if (blockingRule == null || rule.index < blockingRule.index) blockingRule = rule
                    }
                }
            }
            if (dot < 0) break
            at = dot + 1
        }
        // Generic patterns match anywhere in the URL, so the URL is cut into its plain runs
        // and only rules keyed by a run it contains are matched in full; the rest is probed
        // only through the rules with no key of their own.
        val hits = java.util.HashSet<NetworkRule>()
        var runStart = -1
        for (i in rest.indices) {
            val alnum = rest[i].isLetterOrDigit()
            if (alnum && runStart < 0) runStart = i
            if ((!alnum || i == rest.length - 1) && runStart >= 0) {
                val end = if (alnum) i + 1 else i
                val run = rest.substring(runStart, end)
                e.byGoodToken[run]?.let { hits.addAll(it) }
                runStart = -1
            }
        }
        for (rule in hits) {
            if (!rule.matches(host, rest, pageHost)) continue
            if (rule.exception) return false
            if (blockingRule == null || rule.index < blockingRule.index) blockingRule = rule
        }
        for (rule in e.untokened) {
            if (!rule.matches(host, rest, pageHost)) continue
            if (rule.exception) return false
            if (blockingRule == null || rule.index < blockingRule.index) blockingRule = rule
        }
        val rule = blockingRule ?: return false
        lastRule = rule.text
        pageBlockCount.incrementAndGet()
        return true
    }

    /** The URLs of filter lists the user added; callers refresh them on demand. */
    fun customListUrls(): List<String> =
        prefs.getStringSet(KEY_CUSTOM_LISTS, emptySet())?.toList() ?: emptyList()

    fun addCustomListUrl(url: String) {
        val next = customListUrls().toMutableSet()
        next.add(url)
        prefs.edit().putStringSet(KEY_CUSTOM_LISTS, next).apply()
    }

    fun removeCustomListUrl(url: String) {
        val next = customListUrls().toMutableSet()
        next.remove(url)
        prefs.edit().putStringSet(KEY_CUSTOM_LISTS, next).apply()
    }

    /** Re-fetches the user's custom lists and swaps in the rebuilt engine. */
    suspend fun refreshCustomLists(): Result<Unit> = withContext(Dispatchers.IO) {
        runCatching {
            val fetched = customListUrls().map { url ->
                FilterParser.parse(url, fetch(url))
            }
            customLists = fetched
            fetched.forEach { noteListAt(it.name) }
            rebuildEngine()
        }
    }

    /** When a list was last fetched whole, or null when it never was; the Settings screen
     * shows the age and the stale check reads it. */
    fun lastUpdatedOf(file: String): Long? =
        if (prefs.contains("list_updated_$file")) prefs.getLong("list_updated_$file", 0L) else null

    private fun fetch(url: String): String {
        val conn = URL(url).openConnection() as HttpURLConnection
        conn.connectTimeout = 10_000
        conn.readTimeout = 10_000
        try {
            // A page that answers with anything but the list itself would parse as an empty
            // list, so only a clean 200 counts as fetched.
            if (conn.responseCode != 200) throw java.io.IOException("HTTP ${conn.responseCode}")
            return conn.inputStream.bufferedReader().use { it.readText() }
        } finally {
            conn.disconnect()
        }
    }

    @Synchronized
    private fun ensureLoaded() {
        val ctx = context ?: return
        siteExceptions = prefs.getStringSet(KEY_SITE_EXCEPTIONS, emptySet()) ?: emptySet()
        if (loadedBuiltIns) return
        builtIns = Kind.entries.flatMap { kind ->
            kind.files.mapNotNull { name ->
                runCatching {
                    ctx.assets.open("filters/$name").bufferedReader().use { it.readText() }
                }.getOrNull()?.let { FilterParser.parse(name, it, kind = kind.label) }
            }
        }
        loadedBuiltIns = true
        rebuildEngine()
    }

    /** Parsed lists can be installed directly, which is how tests drive the blocker. */
    @Synchronized
    fun replaceLists(lists: List<FilterList>) {
        builtIns = lists
        loadedBuiltIns = true
        rebuildEngine()
    }

    @Synchronized
    private fun rebuildEngine() {
        val byToken = HashMap<String, MutableList<NetworkRule>>()
        val byGoodToken = HashMap<String, MutableList<NetworkRule>>()
        val untokened = ArrayList<NetworkRule>()
        val kindsOn = Kind.entries.filter { kindOn(it) }.map { it.label }.toSet()
        var index = 0
        for (list in builtIns + customLists) {
            // A kind turned off leaves its rules out, and a user's own list runs for every kind.
            if (list.kind != null && list.kind !in kindsOn) continue
            for (rule in list.networkRules) {
                // The engine's own order, not each list's, is what the matcher reports.
                val rule = if (rule.index == index) rule else rule.copy(index = index)
                index++
                if (rule.domainTokens.isEmpty()) {
                    val token = goodTokenOf(rule)
                    if (token == null) untokened.add(rule)
                    else byGoodToken.getOrPut(token) { ArrayList() }.add(rule)
                } else {
                    for (token in rule.domainTokens) {
                        byToken.getOrPut(token) { ArrayList() }.add(rule)
                    }
                }
            }
        }
        engine = Engine(byToken, byGoodToken, untokened, HideStyles.build(builtIns + customLists, kindsOn))
    }

    /** The rule's longest plain run that no pattern edge and no run edge touches:
     * '^' is a boundary in the URL too, so a matching URL carries the run as a whole
     * plain run, and it can key the rule. Null when no such run exists. */
    private fun goodTokenOf(rule: NetworkRule): String? {
        val body = rule.pattern
        var best = ""
        var start = -1
        var i = 0
        while (i < body.length) {
            if (body[i].isLetterOrDigit()) {
                if (start < 0) start = i
            } else {
                // Only a run with a non-alphanumeric char on both sides keys the rule:
                // a matching URL must carry it as a whole run, never inside a longer one.
                if (start > 0 && (i - start) > best.length) best = body.substring(start, i)
                start = -1
            }
            i++
        }
        return best.ifEmpty { null }
    }

    /** Turns a kind's rules on or off and swaps the rebuilt engine in. */
    @Synchronized
    fun setKind(kind: Kind, on: Boolean) {
        setKindOn(kind, on)
        rebuildEngine()
    }

    /** Writes when a list last fetched well, so the submenu can say it and call one stale. */
    private fun noteListAt(name: String) {
        prefs.edit().putLong("list_updated_$name", System.currentTimeMillis()).apply()
        _listTimes.value = listTimes()
    }

    private fun listTimes(): Map<String, Long> =
        Kind.entries.flatMap { it.files }.associateWith { listAt(it) }

    /** When a built-in list last fetched well, 0 when it never has: the shipped seed. */
    fun listAt(name: String): Long =
        if (prefs.contains("list_updated_$name")) prefs.getLong("list_updated_$name", 0L) else 0L

    private val _listTimes = kotlinx.coroutines.flow.MutableStateFlow(listTimes())
    val listTimesFlow = _listTimes

    /** A list is stale once a week has gone by since it last fetched, as filters age fast. */
    fun listStale(name: String, now: Long = System.currentTimeMillis()): Boolean =
        now - listAt(name) > 7L * 24 * 3600 * 1000

    /** Re-fetches the built-in lists from their pins and swaps in the rebuilt engine; the
     * assets' seeds are the fallback when a fetch fails, so a bad network never empties the
     * engine. */
    suspend fun updateBuiltIns(): Result<Unit> = withContext(Dispatchers.IO) {
        runCatching {
            val ctx = context ?: return@runCatching
            val fetched = Kind.entries.flatMap { kind ->
                kind.files.mapNotNull { name ->
                    val url = fetchUrls[name] ?: return@mapNotNull null
                    val body = runCatching { fetch(url) }.getOrNull()
                    if (body != null) noteListAt(name)
                    val seed = body
                        ?: ctx.assets.open("filters/$name").bufferedReader().use { it.readText() }
                    FilterParser.parse(name, seed, kind = kind.label)
                }
            }
            synchronized(this) {
                builtIns = fetched
                loadedBuiltIns = true
                rebuildEngine()
            }
        }
    }

    /** Runs once per launch, off the launch path: a stale list fetches again, so the user never
     * waits on it and the lists stay no older than a week while the app keeps being used. */
    suspend fun updateIfStale() {
        val stale = Kind.entries.flatMap { it.files }.filter { listStale(it) }
        if (stale.isNotEmpty()) updateBuiltIns().getOrNull()
    }

    private fun NetworkRule.matches(host: String, rest: String, pageHost: String?): Boolean {
        // blocks() carries no resource type, so type-constrained rules match on pattern alone
        if (thirdParty != null) {
            if (pageHost == null) return false
            val third = !sameSite(host, pageHost)
            if (third != thirdParty) return false
        }
        if (includeDomains.isNotEmpty() || excludeDomains.isNotEmpty()) {
            if (pageHost == null) return false
            if (excludeDomains.any { domainMatches(pageHost, it) }) return false
            if (includeDomains.isNotEmpty() && includeDomains.none { domainMatches(pageHost, it) }) return false
        }
        val hostPart = this.hostPart
        val hostTail = this.hostTail
        return when {
            anchorStart -> {
                val atHost = host == hostPart || host.endsWith(".$hostPart")
                atHost && tailMatches(hostTail, rest, host.length)
            }
            anchorEnd -> suffixMatches(pattern, rest)
            else -> substringMatches(this, rest)
        }
    }

    /** `rest` is the URL after "://", `at` the offset where the tail must start. */
    private fun tailMatches(tail: String, rest: String, at: Int): Boolean {
        var i = at
        var idx = 0
        while (idx < tail.length) {
            val ch = tail[idx]
            if (ch == '^') {
                // ^ stands for a boundary or the end of the URL, but only when nothing follows
                if (i >= rest.length) {
                    var k = idx + 1
                    while (k < tail.length && tail[k] == '^') k++
                    if (k < tail.length) return false
                    return true
                }
                if (rest[i].isLetterOrDigit()) return false
            } else {
                if (i >= rest.length || rest[i] != ch) return false
            }
            i++
            idx++
        }
        return true
    }

    private fun substringMatches(rule: NetworkRule, rest: String): Boolean {
        val firstSegment = rule.segment
        val tail = rule.segmentTail
        if (firstSegment.isEmpty()) return tailMatches(tail, rest, 0)
        var from = 0
        while (true) {
            val at = rest.indexOf(firstSegment, from)
            if (at < 0) return false
            if (tailMatches(tail, rest, at + firstSegment.length)) return true
            from = at + 1
        }
    }

    private fun suffixMatches(pattern: String, rest: String): Boolean {
        // walk the pattern backwards so ^ can stand for any boundary at the URL's end
        var i = rest.length
        var j = pattern.length
        while (j > 0) {
            val ch = pattern[--j]
            if (ch == '^') {
                if (i == rest.length) continue  // ^ consumed by the URL end
                i--
                if (i < 0 || rest[i].isLetterOrDigit()) return false
            } else {
                i--
                if (i < 0 || rest[i] != ch) return false
            }
        }
        return i >= 0
    }

    private fun domainMatches(host: String, domain: String): Boolean =
        host == domain || host.endsWith(".$domain")

    /** Two hosts are same-site when one is the other or a subdomain of it. */
    private fun sameSite(a: String, b: String): Boolean =
        a == b || a.endsWith(".$b") || b.endsWith(".$a")

    companion object {
        private const val PREFS_FILE = "blocking"
        private const val KEY_ON = "blocking_on"
        private const val KEY_HIDES = "blocking_hides"
        private const val KEY_SITE_EXCEPTIONS = "blocking_site_exceptions"
        private const val KEY_CUSTOM_LISTS = "blocking_custom_lists"

        fun hostOf(url: String): String? {
            val scheme = url.indexOf("://")
            if (scheme <= 0) return null
            var end = url.indexOf('/', scheme + 3)
            if (end < 0) end = url.length
            var at = url.indexOf('@', scheme + 3)
            if (at in (scheme + 3) until end) at += 1 else at = scheme + 3
            val host = url.substring(at, end)
            val colon = host.indexOf(':')
            return if (colon > 0) host.substring(0, colon) else host
        }

        private fun afterScheme(url: String, host: String): String {
            val at = url.indexOf(host)
            return if (at >= 0) url.substring(at) else url
        }
    }
}
