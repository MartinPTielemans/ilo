package app.ilo.browser.android.blocking

/**
 * The element-hiding half of the filter lists, precomputed at engine rebuild: a generic
 * stylesheet that runs on every page, a map of domains to their selectors, and the
 * exceptions that take both back. Selector frequency decides what a size budget cuts, so
 * the sheet keeps the rules a page most often meets. Scriptlets are counted, never run.
 */
object HideStyles {

    /** The generic sheet stays under this, so a page never pays a megabyte of CSS for hides. */
    const val GENERIC_BUDGET = 150 * 1024

    /** What a page is told to hide: the generic sheet plus its own host's selectors. */
    data class Styles(
        val genericCss: String,
        val hostCss: String,
        val hostSelectors: Int,
        val genericSelectors: Int,
    )

    /** One hide a domain names, with the rule's own excluded subdomains kept. */
    class HostHide(val selector: String, val excluded: List<String>)

    /** The built hide state: the generic sheet, per-domain selectors, and what was skipped. */
    data class Built(
        val genericCss: String,
        val genericSelectors: List<String>,
        val byHost: Map<String, List<HostHide>>,
        /** Selectors a host's #@# rules take out of the generic sheet for it and its subdomains. */
        val hostExceptions: Map<String, Set<String>>,
        val genericUsed: Int,
        val genericSkipped: Int,
        val scriptletsSkipped: Int,
    )

    /** Builds the hide state from the parsed lists, gated by the kinds that run. */
    fun build(lists: List<FilterList>, kindsOn: Set<String>): Built {
        val generic = LinkedHashMap<String, Int>()  // selector -> occurrences across lists
        val perHost = HashMap<String, MutableList<HostHide>>()
        val genericExceptions = HashSet<String>()
        val hostExceptions = HashMap<String, MutableSet<String>>()
        var scriptlets = 0

        for (list in lists) {
            // A kind turned off leaves its hides out; a user's own list runs for every kind.
            if (list.kind != null && list.kind !in kindsOn) continue
            for (rule in list.cosmeticRules) {
                if (rule.scriptlet) { scriptlets++; continue }
                val selector = rule.selector.trim()
                if (selector.isEmpty() || !selectorIsSane(selector)) { scriptlets++; continue }
                val included = rule.domains.filter { !it.startsWith("~") }.map { it.removePrefix(".") }
                val excluded = rule.domains.filter { it.startsWith("~") }
                    .map { it.removePrefix("~").removePrefix(".") }
                if (rule.exception) {
                    // An exception takes its selector back everywhere, or on the domains named.
                    if (included.isEmpty()) genericExceptions.add(selector)
                    else for (d in included) hostExceptions.getOrPut(d) { HashSet() }.add(selector)
                    continue
                }
                if (included.isEmpty()) {
                    generic[selector] = (generic[selector] ?: 0) + 1
                } else {
                    for (d in included) perHost.getOrPut(d) { ArrayList() }.add(HostHide(selector, excluded))
                }
            }
        }

        // The budget keeps the selectors a page most often meets; occurrences say how often.
        val ordered = generic.entries.sortedByDescending { it.value }
        val kept = ArrayList<String>(ordered.size)
        var size = 0
        var skipped = 0
        for ((selector, _) in ordered) {
            if (selector in genericExceptions) { skipped++; continue }
            val cost = selector.length + 2
            if (size + cost > GENERIC_BUDGET) { skipped++; continue }
            kept.add(selector)
            size += cost
        }

        // Per-domain selectors, with each rule's own excluded subdomains kept for lookup.
        val byHost = HashMap<String, List<HostHide>>()
        for ((domain, hides) in perHost) {
            val exceptions = hostExceptions[domain] ?: emptySet()
            val keptHides = hides.filter { it.selector !in exceptions }
            if (keptHides.isNotEmpty()) byHost[domain] = keptHides
        }

        return Built(sheet(kept), kept, byHost, hostExceptions, kept.size, skipped, scriptlets)
    }

    /** A brace or angle bracket in a selector would break out of its group's rule, so a
     * selector carrying one is dropped whole rather than taking its group down. */
    private fun selectorIsSane(selector: String): Boolean =
        !selector.contains('{') && !selector.contains('}') && !selector.contains('<')

    /** The selectors a page at `host` adds to the generic sheet: every domain its host is or
     * sits under names its own, with each rule's excluded subdomains left out. */
    fun hostSelectors(built: Built, host: String): List<String> {
        val out = LinkedHashSet<String>()
        for ((domain, hides) in built.byHost) {
            if (host != domain && !host.endsWith(".$domain")) continue
            for (hide in hides) {
                if (hide.excluded.any { host == it || host.endsWith(".$it") }) continue
                out.add(hide.selector)
            }
        }
        return out.toList()
    }

    /** The stylesheet a page at `host` is given: the generic sheet minus what that host's
     * exceptions take from it, plus its own host's selectors. */
    fun forHost(built: Built, host: String?): Styles {
        val hostSel = host?.let { hostSelectors(built, it) } ?: emptyList()
        val hostCss = sheet(hostSel)
        val generic = if (host == null) built.genericCss else {
            val exceptions = exceptionsFor(built, host)
            if (exceptions.isEmpty()) built.genericCss
            else sheet(built.genericSelectors.filter { it !in exceptions })
        }
        return Styles(generic, hostCss, hostSel.size, genericSelectors(generic))
    }

    /** Every exception selector the host's own domains and their parents name. */
    private fun exceptionsFor(built: Built, host: String): Set<String> {
        val out = HashSet<String>()
        for ((domain, selectors) in built.hostExceptions) {
            if (host == domain || host.endsWith(".$domain")) out.addAll(selectors)
        }
        return out
    }

    private fun sheet(selectors: List<String>): String =
        if (selectors.isEmpty()) "" else selectors.joinToString(",") + "{display:none !important}"

    /** How many selectors the generic sheet carries, read back from its text. */
    fun genericSelectors(genericCss: String): Int =
        if (genericCss.isEmpty()) 0 else genericCss.substringBefore('{').count { it == ',' } + 1
}