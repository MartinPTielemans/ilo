// core/sync_spaces.proto's SpaceEntity. A field keeps whether it was set, since the computer
// compares an entity's bytes with what it would write and commits again on any difference.

package app.ilo.browser.android.sync

class SpaceEntity {
    enum class Kind(val raw: Int) {
        SPACE(0), PIN(1), FOLDER(2), TODAY(3);

        companion object {
            fun of(raw: Int): Kind? = entries.firstOrNull { it.raw == raw }
        }
    }

    var id: String? = null
    var kind: Kind? = null
    var space: String? = null
    var parent: String? = null
    /** A serialized sync_pb.UniquePosition. */
    var position: ByteArray? = null
    var name: String? = null
    var url: String? = null
    var title: String? = null
    var color: Long? = null
    var icon: String? = null
    var theme: String? = null
    /** Seconds since 1970. */
    var seen: Long? = null

    override fun equals(other: Any?): Boolean =
        other is SpaceEntity && other.id == id && other.kind == kind && other.space == space &&
            other.parent == parent && other.position.contentEquals(position) &&
            other.name == name && other.url == url && other.title == title &&
            other.color == color && other.icon == icon && other.theme == theme &&
            other.seen == seen

    override fun hashCode(): Int {
        var hash = id?.hashCode() ?: 0
        hash = hash * 31 + (kind?.hashCode() ?: 0)
        hash = hash * 31 + (space?.hashCode() ?: 0)
        hash = hash * 31 + (parent?.hashCode() ?: 0)
        hash = hash * 31 + (position?.contentHashCode() ?: 0)
        hash = hash * 31 + (name?.hashCode() ?: 0)
        hash = hash * 31 + (url?.hashCode() ?: 0)
        hash = hash * 31 + (title?.hashCode() ?: 0)
        hash = hash * 31 + (color?.hashCode() ?: 0)
        hash = hash * 31 + (icon?.hashCode() ?: 0)
        hash = hash * 31 + (theme?.hashCode() ?: 0)
        hash = hash * 31 + (seen?.hashCode() ?: 0)
        return hash
    }

    companion object {
        const val TODAY_PREFIX = "today:"

        /** The node's key: a Today tab's id without its prefix. */
        fun key(id: String): String =
            if (id.startsWith(TODAY_PREFIX)) id.drop(TODAY_PREFIX.length) else id

        /** An entity and whether every field of it was one of these. One with more was written
         * by a newer ilo, and is never written back. */
        fun parse(bytes: ByteArray): Pair<SpaceEntity, Boolean>? {
            val fields = protoFields(bytes) ?: return null
            val entity = SpaceEntity()
            var understood = true
            for (field in fields) {
                when (field.number) {
                    1, 3, 4, 6, 7, 8, 10, 11 -> {
                        val text = field.string ?: continue.also { understood = false }
                        when (field.number) {
                            1 -> entity.id = text
                            3 -> entity.space = text
                            4 -> entity.parent = text
                            6 -> entity.name = text
                            7 -> entity.url = text
                            8 -> entity.title = text
                            10 -> entity.icon = text
                            else -> entity.theme = text
                        }
                    }
                    2 -> {
                        val value = field.varint
                        // A kind this build doesn't know is kept as an unknown field by the
                        // computer's parser.
                        val raw = value?.toInt()
                        val kind = raw?.let { Kind.of(it) }
                        if (kind != null) entity.kind = kind else understood = false
                    }
                    5 -> entity.position = field.bytes
                    9 -> {
                        val value = field.varint ?: continue
                        entity.color = value.toLong()
                        understood = understood && value <= 0xffffffffuL
                    }
                    12 -> entity.seen = field.varint?.toLong()
                    else -> understood = false
                }
            }
            return entity to understood
        }

        fun isLowercaseUUID(text: String): Boolean {
            val bytes = text.encodeToByteArray()
            if (bytes.size != 36) return false
            for ((i, byte) in bytes.withIndex()) {
                val dash = i == 8 || i == 13 || i == 18 || i == 23
                val hex = byte in 0x30..0x39 || byte in 0x61..0x66
                if (if (dash) byte != 0x2d.toByte() else !hex) return false
            }
            return true
        }
    }

    /** The fields that are set, in their numbers' order, as protobuf writes them. */
    fun serialized(): ByteArray {
        val writer = ProtoWriter()
        id?.let { writer.string(1, it) }
        kind?.let { writer.int(2, it.raw.toLong()) }
        space?.let { writer.string(3, it) }
        parent?.let { writer.string(4, it) }
        position?.let { writer.bytes(5, it) }
        name?.let { writer.string(6, it) }
        url?.let { writer.string(7, it) }
        title?.let { writer.string(8, it) }
        color?.let { writer.varint(9, it.toULong()) }
        icon?.let { writer.string(10, it) }
        theme?.let { writer.string(11, it) }
        seen?.let { writer.int(12, it) }
        return writer.toByteArray()
    }

    /** What core/sync_spaces.cc's Parse asks of an entity before it takes it. */
    val isValid: Boolean
        get() {
            val id = id ?: return false
            return id.startsWith(TODAY_PREFIX) == (kind == Kind.TODAY) &&
                isLowercaseUUID(SpaceEntity.key(id))
        }
}