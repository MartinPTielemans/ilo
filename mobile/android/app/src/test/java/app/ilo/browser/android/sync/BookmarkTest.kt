package app.ilo.browser.android.sync

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class BookmarkTest {

    private val key = NigoriKey(
        ByteArray(16) { (it * 3).toByte() }, ByteArray(16) { (it * 5 + 1).toByte() })
    private val keys = Keybag(mapOf(key.name to key), key.name)

    @Test
    fun readsABookmarkAsChromiumWritesOne() {
        // BookmarkSpecifics by its field numbers: url 1, legacy title 3, creation_time_utc 4,
        // guid 10, full_title 11, parent_guid 14, type 15, unique_position 16.
        val position = UniquePosition.initial(UniquePosition.randomSuffix())
        val data = ProtoWriter()
        data.string(1, "https://example.org/")
        data.string(3, "Example")
        data.int(4, 13_400_000_000_000_000)
        data.string(10, "11111111-2222-3333-4444-555555555555")
        data.string(11, "Example page")
        data.string(14, BookmarkFolders.MOBILE)
        data.int(15, 1)
        data.bytes(16, position.proto())
        val bookmark = BookmarkData(data.toByteArray())
        assertEquals("https://example.org/", bookmark.url)
        assertEquals("Example page", bookmark.title)
        assertEquals(13_400_000_000_000_000, bookmark.added)
        assertEquals("11111111-2222-3333-4444-555555555555", bookmark.guid)
        assertEquals(BookmarkFolders.MOBILE, bookmark.parent)
        assertFalse(bookmark.isFolder)
        assertArrayEquals(position.compressed, bookmark.position?.let {
            UniquePosition.fromProto(it).compressed
        })
        assertTrue(bookmark.takes)
    }

    @Test
    fun onlyAMobileFolderPageIsKept() {
        fun takes(build: ProtoWriter.() -> Unit): Boolean {
            val data = ProtoWriter()
            data.build()
            return BookmarkData(data.toByteArray()).takes
        }
        assertTrue(takes {
            string(1, "https://example.org/"); string(10, "a-guid")
            string(14, BookmarkFolders.MOBILE)
        })
        // A folder, whatever else it carries.
        assertFalse(takes {
            string(10, "a-guid"); string(14, BookmarkFolders.MOBILE); int(15, 2)
        })
        // A page of another folder, as each of the phone's lane rules names.
        assertFalse(takes {
            string(1, "https://example.org/"); string(10, "a-guid")
            string(14, BookmarkFolders.BOOKMARK_BAR)
        })
        assertFalse(takes {
            string(1, "https://example.org/"); string(10, "a-guid")
            string(14, BookmarkFolders.OTHER)
        })
        // No url or no guid of its own.
        assertFalse(takes { string(10, "a-guid"); string(14, BookmarkFolders.MOBILE) })
        assertFalse(takes {
            string(1, "https://example.org/"); string(14, BookmarkFolders.MOBILE)
        })
    }

    @Test
    fun writesABookmarkAsADesktopAddsOne() {
        // CreateSpecificsFromBookmarkNode for a page with no favicon.
        val now = 1_800_000_000_000L
        val data = BookmarkData.new(
            "https://example.org/", "Example page", "11111111-2222-3333-4444-555555555555", now)
        val fields = protoFields(data)!!
        assertEquals(listOf(1, 3, 4, 10, 11, 14, 15), fields.map { it.number })
        assertEquals("https://example.org/", fields.lastOrNull { it.number == 1 }?.string)
        assertEquals("Example page", fields.lastOrNull { it.number == 3 }?.string)
        assertEquals("Example page", fields.lastOrNull { it.number == 11 }?.string)
        // Microseconds since 1601, the same epoch PasswordData.time uses.
        assertEquals(11_644_473_600_000_000L, BookmarkData.time(0))
        assertEquals(
            BookmarkData.time(now).toULong(), fields.lastOrNull { it.number == 4 }?.varint)
        assertEquals(1uL, fields.lastOrNull { it.number == 15 }?.varint)
        assertEquals(BookmarkFolders.MOBILE, fields.lastOrNull { it.number == 14 }?.string)
    }

    @Test
    fun namesABookmarkAsTheServerDoes() {
        // ClientTagHash::FromUnhashed(BOOKMARKS, guid): the sha1 of the type's
        // EntitySpecifics field followed by the tag, base64.
        val guid = "11111111-2222-3333-4444-555555555555"
        assertEquals(
            clientTagHash(type = DataType.BOOKMARKS, tag = guid), BookmarkData.tagHash(guid))
        assertFalse(BookmarkData.tagHash(guid) == BookmarkData.tagHash(guid.reversed()))
    }

    @Test
    fun aBookmarksCommitEntry() {
        // CommitContributionImpl::PopulateCommitProto for BOOKMARKS: unique_position 25 and
        // folder 22 on the entity, the specifics in EntitySpecifics.encrypted 21/1 with the
        // type's own field 32904 empty.
        val data = BookmarkData.new("https://example.org/", "Example", "a-guid", 1_700_000_000_000)
        val position = UniquePosition.initial(UniquePosition.randomSuffix()).proto()
        val write = BookmarkWrite(
            EncryptedData(key.name, key.encrypt(data)), base = 0, saved = 5, position = position)
        val entry = CommitEntry(
            id = "an-id", hash = BookmarkData.tagHash("a-guid"), version = 0,
            created = 5, modified = 6, encrypted = null, bookmark = write, folder = false)
        val writer = ProtoWriter()
        entry.write(writer)
        val fields = protoFields(writer.toByteArray())!!
        assertEquals(listOf(1, 4, 5, 6, 7, 18, 22, 25, 21, 23), fields.map { it.number })
        assertArrayEquals(position, fields.lastOrNull { it.number == 25 }?.bytes)
        val specifics = protoFields(fields.lastOrNull { it.number == 21 }?.bytes!!)!!
        assertEquals(
            listOf(1, DataType.BOOKMARKS), specifics.map { it.number })
        assertArrayEquals(data, keys.decrypt(EncryptedData.parse(specifics[0].bytes!!)!!))
        assertArrayEquals(ByteArray(0), specifics[1].bytes)
        // A bookmark's entity read back takes it for one, with its ciphertext.
        val update = SyncUpdate.parse(writer.toByteArray())!!
        assertTrue(update.isBookmark)
        assertFalse(update.isSpace)
        assertFalse(update.isPassword)
        assertEquals(key.name, update.bookmark?.keyName)
        assertArrayEquals(data, keys.decrypt(update.bookmark!!))
        // A deletion carries no position and no specifics of its own.
        val removal = CommitEntry(
            id = "an-id", hash = "a-hash", version = 7, created = 5, modified = 6,
            encrypted = null, bookmark = BookmarkWrite(null, 7, 5, ByteArray(0)))
        val removalWriter = ProtoWriter()
        removal.write(removalWriter)
        val removalFields = protoFields(removalWriter.toByteArray())!!
        assertEquals(listOf(1, 4, 5, 7, 18, 23), removalFields.map { it.number })
        assertTrue(removal.deleted)
        assertEquals("", removalFields.lastOrNull { it.number == 7 }?.string)
    }

    @Test
    fun readsABookmarkEntityAsGetUpdatesGivesOne() {
        val data = BookmarkData.new("https://example.org/", "Example", "a-guid", 1_700_000_000_000)
        val entity = ProtoWriter()
        entity.string(1, "server-id")
        entity.int(4, 12)
        entity.string(7, "encrypted")
        entity.message(21) {
            message(1) {
                EncryptedData(key.name, key.encrypt(data)).write(this)
            }
            bytes(DataType.BOOKMARKS, ByteArray(0))
        }
        entity.string(23, BookmarkData.tagHash("a-guid"))
        val update = SyncUpdate.parse(entity.toByteArray())!!
        assertTrue(update.isBookmark)
        assertNull(update.password)
        assertEquals(key.name, update.bookmark?.keyName)
        val bookmark = BookmarkData(keys.decrypt(update.bookmark!!)!!)
        assertEquals("https://example.org/", bookmark.url)
        assertEquals("Example", bookmark.title)
        // Another key reads nothing of it.
        val other = Keybag(mapOf("other" to NigoriKey(ByteArray(16) { 9 }, ByteArray(16) { 8 })), "other")
        assertNull(other.decrypt(update.bookmark!!))
    }
}