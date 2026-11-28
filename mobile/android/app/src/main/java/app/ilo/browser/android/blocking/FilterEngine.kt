package app.ilo.browser.android.blocking

/**
 * What a page or request is matched against: the filter lists the app blocks with.
 * Implementations must be safe to call from WebView's request callbacks (any thread).
 */
interface Blocker {
    /** Whether ads and trackers are kept from `url`, loaded by the page at `pageUrl`. */
    fun blocks(url: String, pageUrl: String?): Boolean

    /** The same answer with a tab's own held choice for the page's site run over the site's:
     * false keeps nothing from the page, true runs the rules with the site's exception set
     * aside, null runs the site's own choice. The master switch still wins over both. */
    fun blocks(url: String, pageUrl: String?, heldChoice: Boolean?): Boolean =
        if (heldChoice == false) false else blocks(url, pageUrl)

    /** Which rule blocked last, for the blocked count in the UI, or null when nothing did. */
    val lastRule: String?

    /** Whether blocking is on at all. */
    val isOn: Boolean
}