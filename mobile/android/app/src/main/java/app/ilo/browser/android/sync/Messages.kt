// Sync's messages (components/sync/protocol/sync.proto), the fields of them the phone sends and
// reads, by their numbers there.

package app.ilo.browser.android.sync

object DataType {
    /** EntitySpecifics' field for each type, which is also its id in a progress marker. */
    const val NIGORI = 47745
    const val PASSWORDS = 45873
    const val BOOKMARKS = 32904
    const val SESSIONS = 50119
    const val ILO_SPACE = 500_000_001
}

/** What names an entity to the server: the hash of its type and its client tag, which for a
 * Space's entity is its id (components/sync/base/client_tag_hash.cc). */
fun clientTagHash(type: Int = DataType.ILO_SPACE, tag: String): String {
    val specifics = ProtoWriter()
    specifics.bytes(type, ByteArray(0))
    return Crypto.sha1(specifics.toByteArray() + tag.encodeToByteArray()).base64()
}

/** Milliseconds since 1970, sync's time. */
fun protoTime(): Long = System.currentTimeMillis()

/** One entry of a commit, as Chromium writes one for a type that's encrypted
 * (CommitContributionImpl::PopulateCommitProto): for ILO_SPACE as DataTypeWorker's
 * EncryptSpecifics leaves it. */
class CommitEntry(
    val id: String,
    val hash: String,
    /** The version this change was made on: 0 for a new entity. */
    val version: Long,
    val created: Long,
    val modified: Long,
    /** Its SpaceEntity encrypted, or null to delete it. */
    val encrypted: EncryptedData?,
    /** A password's own specifics, which take the place of `encrypted`: a password is encrypted
     * there, as EncryptPasswordSpecificsData leaves it. */
    val password: PasswordWrite? = null,
    /** A bookmark's own entity fields, as CommitContributionImpl::PopulateCommitProto writes
     * them for BOOKMARKS: unique_position and folder beside the specifics. */
    val bookmark: BookmarkWrite? = null,
    /** SyncEntity's `folder`, which bookmarks still carry for clients before M99. */
    val folder: Boolean = false,
) {
    val deleted: Boolean get() = encrypted == null && password == null && bookmark?.deleted != false

    fun write(writer: ProtoWriter) {
        writer.string(1, id)
        writer.int(4, version)
        writer.int(5, modified)
        if (!deleted) writer.int(6, created)
        // The real name would tell the server what's inside. A deletion's is empty.
        writer.string(7, if (deleted) "" else "encrypted")
        writer.bool(18, deleted)
        if (!deleted) {
            writer.bool(22, folder)
            // Only a live bookmark entity carries a position, as PopulateCommitProto writes it.
            bookmark?.let { write -> writer.bytes(25, write.position) }
        }
        if (deleted && bookmark != null) {
            // A deleted bookmark entity carries no specifics at all, as Chromium commits one.
        } else writer.message(21) {
            encrypted?.let { message(1, it::write) }
            password?.let { write ->
                message(DataType.PASSWORDS) {
                    message(1) { write.encrypted.write(this) }
                    message(5) { write.notes.write(this) }
                }
            }
            bookmark?.let { write ->
                if (write.encrypted != null) {
                    // A bookmark's specifics travel in EntitySpecifics.encrypted, as every
                    // type but passwords does: the whole BookmarkSpecifics encrypted there.
                    message(1, write.encrypted::write)
                }
                // The type's own field, empty, says which type the entity is.
                bytes(DataType.BOOKMARKS, ByteArray(0))
            }
            if (password == null && bookmark == null) {
                // The type's own field, empty, says which type the entity is.
                bytes(DataType.ILO_SPACE, ByteArray(0))
            }
        }
        writer.string(23, hash)
    }

    companion object {
        /** What `encrypted` holds before it's encrypted: the entity's EntitySpecifics. */
        fun plaintext(entity: SpaceEntity): ByteArray {
            val specifics = ProtoWriter()
            specifics.message(DataType.ILO_SPACE) { bytes(1, entity.serialized()) }
            return specifics.toByteArray()
        }
    }
}

object SyncMessage {
    private const val PROTOCOL_VERSION = 99uL

    private fun envelope(
        share: String, birthday: String, contents: ULong, body: (ProtoWriter) -> Unit
    ): ByteArray {
        val message = ProtoWriter()
        message.string(1, share)
        message.varint(2, PROTOCOL_VERSION)
        message.varint(3, contents)
        body(message)
        if (birthday.isNotEmpty()) message.string(7, birthday)
        return message.toByteArray()
    }

    fun commit(share: String, birthday: String, cacheGUID: String, entries: List<CommitEntry>): ByteArray =
        envelope(share, birthday, 1uL) { message ->
            message.message(4) {
                for (entry in entries) message(1, entry::write)
                string(2, cacheGUID)
            }
        }

    /** `markers` are each type's id and how far the phone has it, empty for none of it. */
    fun getUpdates(share: String, birthday: String, markers: List<Pair<Int, ByteArray>>): ByteArray =
        envelope(share, birthday, 2uL) { message ->
            message.message(5) {
                bool(3, true)
                for ((type, token) in markers) {
                    message(6) {
                        int(1, type.toLong())
                        bytes(2, token)
                    }
                }
                // NEW_CLIENT for a first download, GU_TRIGGER after.
                varint(9, if (markers.any { it.second.isEmpty() }) 9uL else 12uL)
            }
        }
}

/** An entity as GetUpdates gives it. */
class SyncUpdate private constructor(bytes: ByteArray) {
    var id: String = ""
    var version: Long = 0
    var created: Long = 0
    var deleted: Boolean = false
    var serverTag: String? = null
    var hash: String? = null
    var encrypted: EncryptedData? = null
    /** NigoriSpecifics, on the account's Nigori. */
    var nigori: ByteArray? = null
    /** Whether the entity is one of ILO_SPACE. */
    var isSpace: Boolean = false
    /** Whether the entity is one of PASSWORDS, and its PasswordSpecifics' own `encrypted`: a
     * password is encrypted there, never in the specifics' as other types are. */
    var isPassword: Boolean = false
    var password: EncryptedData? = null
    /** Whether the entity is one of BOOKMARKS, and its BookmarkSpecifics, which its
     * EntitySpecifics.encrypted holds as other types' do. */
    var isBookmark: Boolean = false
    var bookmark: EncryptedData? = null
    /** Whether the entity is one of SESSIONS, whose SessionSpecifics sits in the specifics'
     * encrypted like other types' do. */
    var isSessions: Boolean = false

    companion object {
        fun parse(bytes: ByteArray): SyncUpdate? {
            val fields = protoFields(bytes) ?: return null
            val update = SyncUpdate(bytes)
            update.id = fields.last(1)?.string ?: ""
            update.version = fields.last(4)?.varint?.toLong() ?: 0
            update.created = fields.last(6)?.varint?.toLong() ?: 0
            update.deleted = fields.last(18)?.varint == 1uL
            update.serverTag = fields.last(10)?.string
            update.hash = fields.last(23)?.string
            val specifics = fields.last(21)?.bytes?.let { protoFields(it) }
            if (specifics != null) {
                update.encrypted = EncryptedData.parse(specifics.last(1)?.bytes)
                update.nigori = specifics.last(DataType.NIGORI)?.bytes
                update.isSpace = specifics.last(DataType.ILO_SPACE) != null
                specifics.last(DataType.PASSWORDS)?.bytes?.let { protoFields(it) }?.let { own ->
                    update.isPassword = true
                    update.password = EncryptedData.parse(own.last(1)?.bytes)
                }
                if (specifics.last(DataType.BOOKMARKS) != null) {
                    update.isBookmark = true
                    update.bookmark = update.encrypted
                }
                update.isSessions = specifics.last(DataType.SESSIONS) != null
            }
            return update
        }
    }
}

/** ClientToServerResponse. */
class SyncResponse(bytes: ByteArray) {
    enum class ErrorCode(val code: Int) {
        SUCCESS(0), NOT_MY_BIRTHDAY(2), THROTTLED(3), TRANSIENT(8), OBSOLETE(13);
        companion object {
            fun of(code: Int): ErrorCode? = entries.firstOrNull { it.code == code }
        }
    }

    enum class EntryResult(val code: Int) {
        SUCCESS(1), CONFLICT(2), INVALID(4), OVER_QUOTA(5), TRANSIENT(6);
        companion object {
            fun of(code: Int): EntryResult? = entries.firstOrNull { it.code == code }
        }
    }

    class Entry(val result: EntryResult?, val id: String, val version: Long)

    var error: ErrorCode? = null
    var birthday: String = ""
    var throttle: Int? = null
    var updates: List<SyncUpdate> = emptyList()
    var remaining: Int = 0
    var markers: List<Pair<Int, ByteArray>> = emptyList()
    /** A commit's results, one for each entry sent and in their order. */
    var entries: List<Entry> = emptyList()

    init {
        parse(bytes)
    }

    private fun parse(bytes: ByteArray) {
        val fields = protoFields(bytes) ?: return
        // The field's default is UNKNOWN, so a response without one is an error: null here.
        error = fields.last(4)?.varint?.toInt()?.let { ErrorCode.of(it) }
        birthday = fields.last(6)?.string ?: ""
        fields.last(7)?.bytes?.let { protoFields(it) }?.last(5)?.varint?.let {
            throttle = it.toInt()
        }
        fields.last(2)?.bytes?.let { protoFields(it) }?.let { got ->
            val updates = ArrayList<SyncUpdate>()
            val markers = ArrayList<Pair<Int, ByteArray>>()
            for (field in got) {
                when (field.number) {
                    1 -> field.bytes?.let { SyncUpdate.parse(it) }?.let { updates.add(it) }
                    4 -> remaining = field.varint?.toInt() ?: 0
                    5 -> field.bytes?.let { protoFields(it) }?.let { marker ->
                        val type = marker.last(1)?.varint?.toInt()
                        if (type != null) {
                            markers.add(type to (marker.last(2)?.bytes ?: ByteArray(0)))
                        }
                    }
                }
            }
            this.updates = updates
            this.markers = markers
        }
        fields.last(1)?.bytes?.let { protoFields(it) }?.let { commit ->
            // EntryResponse is a group, the only one sync still has.
            val entries = ArrayList<Entry>()
            for (field in commit) {
                if (field.number != 1) continue
                val inner = field.message?.let { protoFields(it) }
                if (inner == null) {
                    this.entries = emptyList()
                    return
                }
                val result = inner.last(2)?.varint?.toInt()?.let { EntryResult.of(it) }
                entries.add(
                    Entry(result, inner.last(3)?.string ?: "", inner.last(6)?.varint?.toLong() ?: 0))
            }
            this.entries = entries
        }
    }
}