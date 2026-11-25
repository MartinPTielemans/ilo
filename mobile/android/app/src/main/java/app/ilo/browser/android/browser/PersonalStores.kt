package app.ilo.browser.android.browser

import java.io.File
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

/**
 * What the phone remembers of a page it was on: its address and its name, kept as one entry
 * so revisits don't stack up, newest first. Private pages never reach it.
 */
@Serializable
data class HistoryEntry(val url: String, val title: String, val at: Long = System.currentTimeMillis())

/** One of the user's bookmarks: a page kept, with its own name. */
@Serializable
data class Bookmark(val url: String, val title: String, val added: Long = System.currentTimeMillis())

/** What a page downloaded, and where it went, as its own record the app can show and open. */
@Serializable
data class DownloadEntry(
    val url: String,
    val fileName: String,
    val mime: String,
    val sizeBytes: Long,
    /** The record's id, the Android download manager's own. */
    val id: Long,
    val at: Long = System.currentTimeMillis(),
)

/** A tab put away rather than closed: everything needed to bring it back. */
@Serializable
data class ArchivedTab(
    val id: String,
    val url: String?,
    val title: String,
    val space: String,
    val faviconColor: Long,
    val wantsDesktop: Boolean = false,
    /** The page's own dark the tab had, as a tab's own choice survives its being put away. */
    val contentDark: Boolean? = null,
    val at: Long = System.currentTimeMillis(),
)

/**
 * The stores the phone keeps for itself, each one file under the app's own directory, each a
 * JSON list written whole on change. History moves its newest to the head and never grows past
 * its cap; the rest append, newest first.
 */
class PersonalStores(private val dir: File) {
    val historyFlow = MutableStateFlow<List<HistoryEntry>>(emptyList())
    val bookmarksFlow = MutableStateFlow<List<Bookmark>>(emptyList())
    val downloadsFlow = MutableStateFlow<List<DownloadEntry>>(emptyList())
    val archivedFlow = MutableStateFlow<List<ArchivedTab>>(emptyList())

    /** The last write that failed, or null when every store wrote or never tried. */
    val persistError = MutableStateFlow<String?>(null)

    private val json = Json { ignoreUnknownKeys = true }
    private val historyFile get() = File(dir, "history.json")
    private val bookmarksFile get() = File(dir, "bookmarks.json")
    private val downloadsFile get() = File(dir, "downloads.json")
    private val archivedFile get() = File(dir, "archived.json")

    init { restore() }

    /** A page finished: its entry moves to the head, older ones of the same page go. */
    fun visit(url: String, title: String) {
        if (url.isBlank()) return
        val kept = historyFlow.value.filterNot { it.url == url }
        historyFlow.value = listOf(HistoryEntry(url, title)) + kept.take(HISTORY_CAP - 1)
        write(historyFile, historyFlow.value)
    }

    fun clearHistory() {
        historyFlow.value = emptyList()
        write(historyFile, historyFlow.value)
    }

    /** The pages visited since `since` go, the older ones stay, as Chrome's own range clears. */
    fun clearHistorySince(since: Long) {
        historyFlow.value = historyFlow.value.filterNot { it.at >= since }
        write(historyFile, historyFlow.value)
    }

    /** One page goes from history, as the panel's typed row X asks, leaving the rest. */
    fun removeHistory(url: String) {
        historyFlow.value = historyFlow.value.filterNot { it.url == url }
        write(historyFile, historyFlow.value)
    }

    /** When the phone was last on the host, as the site controls' row shows it; a page the
     * phone never visited has no answer. */
    fun lastVisitOf(host: String): Long? =
        historyFlow.value.firstOrNull { visitHost(it.url) == host }?.at

    /** The host an entry's address names, as the store's own lookups compare it: the address's
     * own casing lowered and its www dropped, so a page seen at both spellings is one site. */
    private fun visitHost(url: String): String? =
        runCatching { java.net.URI(url).host?.lowercase()?.removePrefix("www.") }.getOrNull()

    fun bookmark(url: String, title: String) {
        if (url.isBlank() || bookmarksFlow.value.any { it.url == url }) return
        bookmarksFlow.value = listOf(Bookmark(url, title)) + bookmarksFlow.value
        write(bookmarksFile, bookmarksFlow.value)
    }

    fun unbookmark(url: String) {
        bookmarksFlow.value = bookmarksFlow.value.filterNot { it.url == url }
        write(bookmarksFile, bookmarksFlow.value)
    }

    /** The whole list at once, as the bookmarks lane replaces it with the account's; an
     * empty one is a valid answer, so no same-list check happens here. */
    fun replaceBookmarks(list: List<Bookmark>) {
        bookmarksFlow.value = list
        write(bookmarksFile, list)
    }

    fun isBookmarked(url: String): Boolean = bookmarksFlow.value.any { it.url == url }

    /** A download began: its record goes in the list, newest first. */
    fun downloadBegan(entry: DownloadEntry) {
        downloadsFlow.value = listOf(entry) + downloadsFlow.value
        write(downloadsFile, downloadsFlow.value)
    }

    /** A download changed or ended: its record keeps its place by id. */
    fun downloadChanged(id: Long, sizeBytes: Long) {
        downloadsFlow.value = downloadsFlow.value.map {
            if (it.id == id) it.copy(sizeBytes = sizeBytes) else it
        }
        write(downloadsFile, downloadsFlow.value)
    }

    fun forgetDownload(id: Long) {
        downloadsFlow.value = downloadsFlow.value.filterNot { it.id == id }
        write(downloadsFile, downloadsFlow.value)
    }

    /** A tab goes to the archive, newest first. */
    fun archive(tab: Tab) {
        archivedFlow.value = listOf(
            ArchivedTab(
                id = tab.id,
                url = tab.url,
                title = tab.title,
                space = tab.space,
                faviconColor = tab.faviconColor,
                wantsDesktop = tab.wantsDesktop,
                contentDark = tab.contentDark,
            ),
        ) + archivedFlow.value
        write(archivedFile, archivedFlow.value)
    }

    /** An archived tab comes back: it leaves the archive, its record returned. */
    fun unarchive(id: String): ArchivedTab? {
        val archived = archivedFlow.value.firstOrNull { it.id == id } ?: return null
        archivedFlow.value = archivedFlow.value.filterNot { it.id == id }
        write(archivedFile, archivedFlow.value)
        return archived
    }

    fun forgetArchived(id: String) {
        archivedFlow.value = archivedFlow.value.filterNot { it.id == id }
        write(archivedFile, archivedFlow.value)
    }

    private fun restore() {
        historyFlow.value = read(historyFile)
        bookmarksFlow.value = read(bookmarksFile)
        downloadsFlow.value = read(downloadsFile)
        archivedFlow.value = read(archivedFile)
    }

    private inline fun <reified T> read(file: File): List<T> =
        runCatching {
            if (file.exists()) json.decodeFromString<List<T>>(file.readText()) else emptyList()
        }.getOrDefault(emptyList())

    /** Inline so T stays reified: kotlinx.serialization can't encode an erased type parameter,
     * and a non-inline write failed every store's file, silently, into persistError. */
    private inline fun <reified T> write(file: File, list: List<T>) {
        persistError.value = try {
            file.parentFile?.mkdirs()
            file.writeText(json.encodeToString(list)); null
        } catch (e: Exception) {
            e.message
        }
    }

    companion object {
        /** How many pages history keeps: the sheet shows a page's worth, the rest is dead weight. */
        const val HISTORY_CAP = 500
    }
}
