package app.ilo.browser.android.browser

import android.content.Context
import android.content.SharedPreferences

/**
 * What a site may do, one answer per kind, kept per host as Chrome keeps its content settings:
 * Ask is the default everywhere, and a site the user answered once keeps its answer across
 * loads. The global submenu shows the same kinds with their default; a site's own row shows
 * the answer it holds.
 */
enum class SitePermission(val label: String) {
    LOCATION("location"),
    CAMERA("camera"),
    MICROPHONE("microphone"),
    NOTIFICATIONS("notifications"),
}

/** One site's answers, the kinds a page can ask for, each unset until it is answered. */
enum class SiteAnswer { ASK, ALLOW, BLOCK }

/** A permission ask standing over a page until it is answered: the host that asked, the kind
 * it asked for, and the two ways the request itself is answered. */
class PendingAsk(
    val host: String,
    val kind: SitePermission,
    val grant: () -> Unit,
    val deny: () -> Unit,
)

/**
 * The permissions the user answered per site, one prefs file as the blocking's own store is,
 * so a relaunch keeps them. A host's answer is read per load and written per answer; the
 * engine's own grant path asks here before it asks the user.
 */
class SitePermissionStore internal constructor(private val prefs: SharedPreferences) {
    constructor(context: Context) : this(context.getSharedPreferences("site_permissions", Context.MODE_PRIVATE))

    /** The host's one spelling in the store: lowered and its www dropped, so a page seen at
     * either address is one site to the engine that asks and the panel that answers. */
    private fun keyOf(host: String) = host.lowercase().removePrefix("www.")

    /** What this host holds for a kind: its own answer, or the kind's global default. */
    fun answer(host: String, kind: SitePermission): SiteAnswer {
        val stored = prefs.getString("${keyOf(host)}:${kind.label}", null)
            ?: return defaultOf(kind)
        return runCatching { SiteAnswer.valueOf(stored) }.getOrDefault(defaultOf(kind))
    }

    /** The kind's own answer, when no site-wide rule decides before the host's own. */
    fun defaultOf(kind: SitePermission): SiteAnswer {
        val stored = prefs.getString("default:${kind.label}", null) ?: return SiteAnswer.ASK
        return runCatching { SiteAnswer.valueOf(stored) }.getOrDefault(SiteAnswer.ASK)
    }

    /** Writes the kind's global default, what every unanswered site starts at. */
    fun setDefault(kind: SitePermission, answer: SiteAnswer) {
        prefs.edit().putString("default:${kind.label}", answer.name).apply()
        _answers.value = readAll()
    }

    /** Writes one answer for one host; null returns the host to asking, as a reset does. */
    fun setAnswer(host: String, kind: SitePermission, answer: SiteAnswer?) {
        val key = "${keyOf(host)}:${kind.label}"
        if (answer == null) prefs.edit().remove(key).apply()
        else prefs.edit().putString(key, answer.name).apply()
        _answers.value = readAll()
    }

    /** Every host with an answer of its own, the global submenu's exceptions list. */
    fun sitesAnswered(): Map<String, Map<SitePermission, SiteAnswer>> = readAll()

    /** Everything a host answers, for its own panel; the kinds unasked come back unset. */
    fun answersFor(host: String): Map<SitePermission, SiteAnswer> =
        SitePermission.entries.associateWith { answer(host, it) }

    /** The answer after `answer` on the panel's row: Ask, Allow, Block, then Ask again, as
     * Chrome's own site row cycles through its three choices. */
    fun nextAnswer(answer: SiteAnswer): SiteAnswer = when (answer) {
        SiteAnswer.ASK -> SiteAnswer.ALLOW
        SiteAnswer.ALLOW -> SiteAnswer.BLOCK
        SiteAnswer.BLOCK -> SiteAnswer.ASK
    }

    /** One host's answers go entirely, as its panel's reset and its clear-browsing-data do. */
    fun clearSite(host: String) {
        for (kind in SitePermission.entries) prefs.edit().remove("${keyOf(host)}:${kind.label}").apply()
        _answers.value = readAll()
    }

    private val _answers = kotlinx.coroutines.flow.MutableStateFlow(readAll())
    val answersFlow = _answers

    private fun readAll(): Map<String, Map<SitePermission, SiteAnswer>> {
        val all = mutableMapOf<String, MutableMap<SitePermission, SiteAnswer>>()
        for ((key, value) in prefs.all) {
            // A host's own answers are "host:kind" (already the store's one spelling) and the
            // global defaults "default:kind" are not a host.
            val split = (key as? String)?.split(":") ?: continue
            if (split.size != 2 || split[0] == "default") continue
            val kind = SitePermission.entries.firstOrNull { it.label == split[1] } ?: continue
            val answer = runCatching { SiteAnswer.valueOf(value as String) }.getOrNull() ?: continue
            all.getOrPut(split[0]) { mutableMapOf() }[kind] = answer
        }
        return all
    }
}
