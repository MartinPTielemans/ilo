// The account's bookmarks as Chromium's BOOKMARKS type writes and reads them
// (components/sync/protocol/bookmark_specifics.proto, bookmark_specifics_conversions.cc),
// named and keyed as its computers do.

package app.ilo.browser.android.sync

/** The permanent folders' GUIDs, as bookmarks::k*NodeUuid keep them. */
object BookmarkFolders {
    const val BOOKMARK_BAR = "0bc5d13f-2cba-5d74-951f-3f233fe6c908"
    const val OTHER = "82b081ec-3dd3-529c-8475-ab6c344590dd"
    const val MOBILE = "4cf2e351-0e85-532b-bb37-df045d8f8d0f"
    // The root's own GUID, named only in a parent_guid nobody else can have.
    const val ROOT = "2509a7dc-215d-52f7-a429-8d80431c6c75"
    val PERMANENT = setOf(BOOKMARK_BAR, OTHER, MOBILE, ROOT)
    /** The GUID whose subtree this phone reads and writes: the mobile one, as Arc's phones do. */
    const val PARENT = MOBILE

    /** The server tags the permanent folders arrive under, as kBookmarkBarTag and its kin. */
    const val MOBILE_TAG = "synced_bookmarks"
    const val OTHER_TAG = "other_bookmarks"
    const val BAR_TAG = "bookmark_bar"
}

/** A bookmark as the account has it, read from BookmarkSpecifics by field number. */
class BookmarkData(bytes: ByteArray) {
    var url: String = ""
    var title: String = ""
    var added: Long = 0
    var guid: String = ""
    var parent: String = ""
    var isFolder: Boolean = false
    var position: ByteArray? = null

    init {
        val fields = try {
            protoFields(bytes)
        } catch (_: Exception) {
            null
        }
        // What isn't a message at all reads as one that fills nothing.
        if (fields != null) {
            for (field in fields) {
                when (field.number) {
                    1 -> url = field.string ?: ""
                    // The legacy title is truncated and escaped; the full one is what Chromium keeps.
                    3 -> if (title.isEmpty()) title = field.string ?: ""
                    4 -> added = field.varint?.toLong() ?: 0
                    10 -> guid = field.string ?: ""
                    11 -> title = field.string ?: ""
                    14 -> parent = field.string ?: ""
                    15 -> isFolder = field.varint == 2uL
                    16 -> position = field.bytes
                }
            }
        }
    }

    /** A page this phone keeps, in a folder it knows, with its own GUID. */
    val takes: Boolean
        get() = !isFolder && url.isNotEmpty() && guid.isNotEmpty() &&
            (parent == BookmarkFolders.PARENT)

    companion object {
        /** Chromium's time for a bookmark: microseconds since 1601. */
        fun time(unixMillis: Long): Long = unixMillis * 1000 + 11_644_473_600_000_000L

        /** A bookmark a phone adds, as bookmark_specifics_conversions.cc's
         * CreateSpecificsFromBookmarkNode writes one for a page with no favicon. */
        fun new(url: String, title: String, guid: String, addedMillis: Long): ByteArray {
            val data = ProtoWriter()
            data.string(1, url)
            data.string(3, title)
            data.int(4, time(addedMillis))
            data.string(10, guid)
            data.string(11, title)
            data.string(14, BookmarkFolders.PARENT)
            // Type.URL = 1, as every one this phone writes is a page.
            data.int(15, 1)
            return data.toByteArray()
        }

        /** The client tag hash that names a bookmark to the server, from its GUID: what
         * ClientTagHash::FromUnhashed(BOOKMARKS, guid) makes, as this phone has it. */
        fun tagHash(guid: String): String = clientTagHash(type = DataType.BOOKMARKS, tag = guid)
    }
}

/** A bookmark entity as the account has it: the whole encrypted specifics, untouched. */
class BookmarkRecord(
    val version: Long,
    val encrypted: EncryptedData,
    /** The server's id for the entity, which a commit that changes it gives back. */
    val id: String?,
    val created: Long?,
    /** The bookmark's own GUID, which also names it to the server. */
    val guid: String,
) {
    override fun equals(other: Any?): Boolean = other is BookmarkRecord && other.version == version &&
        other.encrypted == encrypted && other.id == id && other.created == created &&
        other.guid == guid

    override fun hashCode(): Int =
        (version.hashCode() * 31 + encrypted.hashCode()) * 31 +
            (id?.hashCode() ?: 0) * 31 + (created?.hashCode() ?: 0) * 31 + guid.hashCode()
}

/** A bookmark this phone added or removed and the account hasn't taken yet: its ciphertext
 * only, so the saved state holds no more of it than of any other. */
class BookmarkWrite(
    /** BookmarkSpecifics as EntitySpecifics is: the whole specifics encrypted, as every
     * type but passwords carries it. Null removes the bookmark. */
    val encrypted: EncryptedData?,
    /** The version of the account's entity this was made on: 0 for one it hasn't got. */
    val base: Long,
    /** When the user bookmarked the page, in sync's time. */
    val saved: Long,
    val position: ByteArray,
) {
    val deleted: Boolean get() = encrypted == null

    /** The page this write holds, decrypted for the check a commit makes of it. */
    internal fun keptUrl(keys: Keybag): String? =
        encrypted?.let { keys.decrypt(it) }?.let(::BookmarkData)?.url

    override fun equals(other: Any?): Boolean = other is BookmarkWrite &&
        other.encrypted == encrypted && other.base == base && other.saved == saved &&
        other.position.contentEquals(position)

    override fun hashCode(): Int = (encrypted?.hashCode() ?: 0) * 31 +
        base.hashCode() * 31 + saved.hashCode() * 31 + position.contentHashCode()
}

/** The account's bookmarks as the lane reads and writes them. */
class Bookmarks(private val account: SyncedAccount) {

    /** One bookmark as the lane carries it, local and account fields side by side. */
    data class Kept(val url: String, val title: String, val added: Long, val guid: String)

    /** Every page bookmark this phone can read, as the account has it. An account with no
     * keys has no word on bookmarks: empty would wipe the local store for nothing. */
    fun kept(): List<Kept> {
        if (!account.bookmarksReady()) return emptyList()
        val out = ArrayList<Kept>()
        for (hash in account.bookmarkHashes()) {
            val data = account.bookmarkData(hash) ?: continue
            if (!data.takes) continue
            out.add(Kept(data.url, data.title, data.added, data.guid))
        }
        return out
    }

    /** Bookmarking a page, or nothing when the account can't take it. */
    fun add(url: String, title: String): Boolean {
        if (!account.bookmarksReady()) return false
        val took = account.bookmarkAddress(url) ?: return false
        // A second bookmark of the same page is nothing, as the local store's own rule is.
        if (account.bookmarkHashes().any { hash ->
                account.bookmarkData(hash)?.url == took
            }) return true
        return account.saveBookmark(took, title)
    }

    /** Unbookmarking a page: what the account has for it goes with the next commit. */
    fun remove(url: String): Boolean {
        if (!account.bookmarksReady()) return false
        val took = account.bookmarkAddress(url) ?: return false
        return account.removeBookmark(took)
    }

    /** Whether a page is one the account has, by the address this phone writes. */
    fun has(url: String): Boolean {
        val took = account.bookmarkAddress(url) ?: return false
        return account.bookmarkHashes().any { hash -> account.bookmarkData(hash)?.url == took }
    }
}