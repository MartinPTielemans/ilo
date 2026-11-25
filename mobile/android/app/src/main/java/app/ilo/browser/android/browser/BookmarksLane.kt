package app.ilo.browser.android.browser

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch

/**
 * The bookmarks lane: the only thing that moves bookmarks between the account and the phone's
 * own store. Local bookmarking and unbookmarking push to the account; the account's list
 * replaces the local one when it changes. The local JSON keeps working without an account,
 * as history and downloads do: nothing of it is touched while the account is absent.
 */
class BookmarksLane(
    private val stores: PersonalStores,
    site: Site? = null,
    scope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Default),
) {
    /** What the account can do, so a plain unit test stands in for it. */
    interface Site {
        /** Whether the account's word on bookmarks counts: until the first download it has
         * none, and an empty list then would wipe the phone's own for nothing. */
        val ready: Boolean
        /** The account's bookmarks: url, title, and when it was added, in unix milliseconds. */
        fun kept(): List<Kept>
        /** Bookmarking a page, or nothing when the account can't take it. */
        fun add(url: String, title: String): Boolean
        /** Unbookmarking a page: false when the account hasn't got it. */
        fun remove(url: String): Boolean
        /** Goes up whenever what the account has for bookmarks changed. */
        val tick: StateFlow<Int>
    }

    /** One bookmark as the account keeps it, the fields the local store keeps. */
    data class Kept(val url: String, val title: String, val added: Long)

    /** The account as it stands, watched so an arriving or leaving one is seen at once. */
    private val siteFlow = MutableStateFlow(site)

    /** What the lane last moved, so an unchanged list is never moved again, either way. */
    private var settled = emptyList<Bookmark>()

    /** Whether the account's list has been joined with the phone's own, so bookmarks made
     * before the account arrived are pushed, not read as deleted by its first download. */
    private var joined = false

    init {
        scope.launch { stores.bookmarksFlow.collect { local -> onLocal(local) } }
        scope.launch {
            siteFlow.collectLatest { site ->
                // A leaving account's watch is cancelled; an arriving one is looked at once.
                site?.let {
                    onAccount(site)
                    site.tick.collect { _ -> onAccount(siteFlow.value ?: site) }
                }
            }
        }
    }

    /** The account arrived or left. A different one joins its list with the phone's own. */
    fun setAccount(site: Site?) {
        if (siteFlow.value != site) joined = false
        siteFlow.value = site
    }

    /** Local bookmarks changed: what the user did goes to the account, and what came back
     * from it is not pushed again. */
    private fun onLocal(local: List<Bookmark>) {
        val site = siteFlow.value ?: run { settled = local; return }
        if (same(local)) return
        settled = local
        val theirs = site.kept().map { it.url }.toSet()
        for (bookmark in local) {
            if (bookmark.url !in theirs) site.add(bookmark.url, bookmark.title)
        }
        // What the account has and the store lost was unbookmarked here: it goes.
        val now = local.map { it.url }.toSet()
        for (kept in site.kept()) {
            if (kept.url !in now) site.remove(kept.url)
        }
    }

    /** The account's word applies only once it is ready; until then the phone's own list
     * stands untouched, as it does with no account at all. Once it counts, its bookmarks
     * become the local store's, newest added first, unless the same list is already there. */
    private fun onAccount(site: Site) {
        if (!site.ready) return
        val theirs = site.kept()
        if (!joined) {
            joined = true
            val have = theirs.map { it.url }.toHashSet()
            val local = stores.bookmarksFlow.value
            // The phone's own bookmarks join the account's before its list becomes the word,
            // as tabs do, or the first download would read them as deleted everywhere else.
            for (bookmark in local) {
                if (bookmark.url !in have) site.add(bookmark.url, bookmark.title)
            }
            val wanted = (
                theirs.map { Bookmark(it.url, it.title, it.added) } +
                    local.filter { it.url !in have })
                .sortedByDescending { it.added }
            if (same(wanted)) return
            settled = wanted
            stores.replaceBookmarks(wanted)
            return
        }
        val wanted = theirs.sortedByDescending { it.added }
            .map { Bookmark(it.url, it.title, it.added) }
        if (same(wanted)) return
        settled = wanted
        stores.replaceBookmarks(wanted)
    }

    /** Equal in every field the store keeps, so an unchanged list never moves. */
    private fun same(list: List<Bookmark>): Boolean =
        list.size == settled.size && list.zip(settled).all { (one, other) ->
            one.url == other.url && one.title == other.title
        }
}