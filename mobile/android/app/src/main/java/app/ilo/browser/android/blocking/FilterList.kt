package app.ilo.browser.android.blocking

/** Resource kinds a network rule can constrain; anything unmapped lands in OTHER. */
enum class ResourceType { MAIN_FRAME, SUB_FRAME, SCRIPT, IMAGE, STYLESHEET, XHR, OTHER }

/** A parsed network rule: what it matches, where it must run, and what it allows or blocks. */
data class NetworkRule(
    val text: String,
    /** Parse order within the engine, so the earliest rule a request matches is reported
     * however the candidate sets iterate. */
    val index: Int,
    val pattern: String,
    /** The pattern before '^' and '/', and what follows it: the anchored-host comparison's
     * slices, precomputed so matching never re-slices the pattern. */
    val hostPart: String,
    val hostTail: String,
    /** The pattern before '^', and what follows it: the anywhere-in-URL probe's slices. */
    val segment: String,
    val segmentTail: String,
    val exception: Boolean,
    val anchorStart: Boolean,  // || : match from the start of the host
    val anchorEnd: Boolean,    // | : match to the end of the URL
    val domainTokens: List<String>,  // labels of an anchored domain, used to index the rule
    val includeDomains: List<String>,
    val excludeDomains: List<String>,
    val types: Set<ResourceType>?,  // null means every type
    val thirdParty: Boolean?,      // null means either
) {
    fun matchesType(type: ResourceType) = types == null || type in types
}

/** A cosmetic rule: the domains it runs on (empty means every page), its selector, and whether
 * it is an exception. Scriptlets and other injections parse but never run in a page. */
data class CosmeticRule(
    val text: String,
    val selector: String,
    val exception: Boolean,
    val domains: List<String> = emptyList(),
    val scriptlet: Boolean = false,
)

/** The outcome of parsing one filter list. */
data class FilterList(
    val name: String,
    val networkRules: List<NetworkRule>,
    val cosmeticRules: List<CosmeticRule>,
    /** Which kind of blocking this list serves: one of EasyListBlocker.Kind's labels, or null
     * for a list the user added, which runs for every kind. */
    val kind: String? = null,
)