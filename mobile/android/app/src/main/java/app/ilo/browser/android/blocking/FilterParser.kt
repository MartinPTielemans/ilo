package app.ilo.browser.android.blocking

/**
 * Parses EasyList-format text. Anything it doesn't understand is skipped, never fatal:
 * a bad rule in a downloaded list must not take the whole list down.
 */
object FilterParser {

    private val typeOptions = mapOf(
        "document" to ResourceType.MAIN_FRAME,
        "main_frame" to ResourceType.MAIN_FRAME,
        "subdocument" to ResourceType.SUB_FRAME,
        "sub_frame" to ResourceType.SUB_FRAME,
        "script" to ResourceType.SCRIPT,
        "image" to ResourceType.IMAGE,
        "stylesheet" to ResourceType.STYLESHEET,
        "xmlhttprequest" to ResourceType.XHR,
        "xhr" to ResourceType.XHR,
    )

    fun parse(name: String, text: String, kind: String? = null): FilterList {
        val network = ArrayList<NetworkRule>()
        val cosmetic = ArrayList<CosmeticRule>()
        for (rawLine in text.lineSequence()) {
            val line = rawLine.trim()
            if (line.isEmpty() || line.startsWith("!") || line.startsWith("[")) continue
            val marker = listOf("##", "#@#", "#?", "#$#").firstOrNull { line.contains(it) }
            if (marker != null || line.contains(":style(")) {
                val exception = line.contains("#@#")
                val at = line.indexOfFirst { it == '#' }
                // the selector starts after the marker, and EasyList markers run 2-3 chars
                val selector = line.substring(at + marker!!.length)
                // +js, HTML filters and injected CSS run code or whole stylesheets, so they
                // are never put in a page; they stay counted as what a list asked for.
                val scriptlet = selector.startsWith("+js(") || selector.startsWith("^") ||
                    marker == "#$#" || line.contains(":style(")
                val domains = line.take(at).split(',').map { it.trim().lowercase() }.filter { it.isNotEmpty() }
                cosmetic.add(CosmeticRule(line, selector, exception, domains, scriptlet))
                continue
            }
            parseNetwork(line, network.size)?.let(network::add)
        }
        return FilterList(name, network, cosmetic, kind)
    }

    // A path rule like /ads/ is plain text; a regex rule carries regex markers inside the slashes.
    private fun isRegex(pattern: String): Boolean {
        if (pattern.length < 3 || !pattern.startsWith("/") || !pattern.endsWith("/")) return false
        val inner = pattern.substring(1, pattern.length - 1)
        return inner.startsWith("^") || inner.endsWith("$") || inner.contains("\\\\") || inner.contains("(?")
    }

    /** Returns null for rules we don't support (regex, unknown options, HTML filters). */
    private fun parseNetwork(line: String, index: Int): NetworkRule? {
        var body = line
        val exception = body.startsWith("@@")
        if (exception) body = body.substring(2)

        var pattern = body
        var options: String? = null
        val dollar = body.lastIndexOf('$')
        if (dollar > 0 && dollar < body.length - 1) {
            // A $ later in the rule separates options only when it's not part of the pattern
            // (dangling-$ anchors don't exist in EasyList; lastIndexOf is the common split).
            pattern = body.substring(0, dollar)
            options = body.substring(dollar + 1)
        }

        var anchorStart = false
        var anchorEnd = false
        if (pattern.startsWith("||")) {
            anchorStart = true
            pattern = pattern.substring(2)
        } else if (pattern.startsWith("|")) {
            anchorStart = true
            pattern = pattern.substring(1)
        }
        if (pattern.endsWith("|") && !pattern.endsWith("|||")) {
            anchorEnd = true
            pattern = pattern.removeSuffix("|")
        }
        // ||example.com| leaves the separator ^ or end; strip a trailing ^ from the pattern
        // because URL boundaries end the host anyway.
        if (pattern.isEmpty()) return null
        if (pattern.contains('*')) {
            // A trailing wildcard means "anything under this path"; interior wildcards are unsupported.
            if (!pattern.endsWith("*") || pattern.dropLast(1).contains('*')) return null
            pattern = pattern.dropLast(1)
        }
        if (isRegex(pattern)) return null

        var includeDomains = emptyList<String>()
        var excludeDomains = emptyList<String>()
        var types: Set<ResourceType>? = null
        var thirdParty: Boolean? = null
        if (options != null) {
            val inc = ArrayList<String>()
            val exc = ArrayList<String>()
            val typeSet = HashSet<ResourceType>()
            for (opt in options.split(',')) {
                val negated = opt.startsWith("~")
                val key = if (negated) opt.substring(1) else opt
                if (key == "third-party" || key == "3p") {
                    thirdParty = !negated
                } else if (key.startsWith("domain=")) {
                    for (d in key.substringAfter('=').split('|')) {
                        val domain = d.removePrefix("~").removePrefix(".").lowercase()
                        if (domain.isEmpty()) continue
                        if (d.startsWith("~")) exc.add(domain) else inc.add(domain)
                    }
                } else if (typeOptions.containsKey(key)) {
                    if (negated) return null  // negated types are rare; skip the rule
                    typeSet.add(typeOptions.getValue(key))
                } else {
                    return null  // unsupported option: drop the rule rather than guess
                }
            }
            includeDomains = inc
            excludeDomains = exc
            types = if (typeSet.isEmpty()) null else typeSet
        }

        val host = if (anchorStart) pattern.substringBefore('^').substringBefore('/') else ""
        val domainTokens = if (host.isNotEmpty()) host.split('.').filter { it.isNotEmpty() } else emptyList()

        val lowered = pattern.lowercase()
        val hostPart = lowered.substringBefore('^').substringBefore('/')
        val segment = lowered.substringBefore('^')
        return NetworkRule(
            text = line,
            index = index,
            pattern = lowered,
            hostPart = hostPart,
            hostTail = lowered.substring(hostPart.length),
            segment = segment,
            segmentTail = lowered.substring(segment.length),
            exception = exception,
            anchorStart = anchorStart,
            anchorEnd = anchorEnd,
            domainTokens = domainTokens,
            includeDomains = includeDomains,
            excludeDomains = excludeDomains,
            types = types,
            thirdParty = thirdParty,
        )
    }
}