// Protocol buffers' wire format, as much of it as sync's messages use: no dependency, and a
// generated codec would hide which fields an entity came with.

package app.ilo.browser.android.sync

/** The wire values a field can carry, as it was read. */
sealed class WireValue {
    class Varint(val value: ULong) : WireValue()
    class Fixed64(val value: ULong) : WireValue()
    class Bytes(val value: ByteArray) : WireValue()
    /** A group's fields, between its start and end tags. */
    class Group(val value: ByteArray) : WireValue()
    class Fixed32(val value: UInt) : WireValue()

    override fun equals(other: Any?): Boolean = other is WireValue && when (this) {
        is Varint -> other is Varint && value == other.value
        is Fixed64 -> other is Fixed64 && value == other.value
        is Bytes -> other is Bytes && value.contentEquals(other.value)
        is Group -> other is Group && value.contentEquals(other.value)
        is Fixed32 -> other is Fixed32 && value == other.value
    }

    override fun hashCode(): Int = when (this) {
        is Varint -> value.hashCode()
        is Fixed64 -> value.hashCode()
        is Bytes -> value.contentHashCode()
        is Group -> value.contentHashCode()
        is Fixed32 -> value.hashCode()
    }
}

class ProtoField(val number: Int, val value: WireValue) {
    val varint: ULong? get() = (value as? WireValue.Varint)?.value

    val bytes: ByteArray? get() = (value as? WireValue.Bytes)?.value

    /** A nested message, which the wire carries as bytes or as a group. */
    val message: ByteArray?
        get() = when (value) {
            is WireValue.Bytes -> value.value
            is WireValue.Group -> value.value
            else -> null
        }

    /** Only valid UTF-8 is a string, as Swift's validating decoder demands. */
    val string: String?
        get() = (value as? WireValue.Bytes)?.value?.utf8OrNull()

    override fun equals(other: Any?): Boolean =
        other is ProtoField && other.number == number && other.value == value

    override fun hashCode(): Int = number * 31 + value.hashCode()
}

private fun ByteArray.utf8OrNull(): String? {
    // Kotlin's decodeToString replaces bad bytes, where Swift's validating decoder refuses them.
    var i = 0
    while (i < size) {
        val b = this[i].toInt() and 0xff
        val extra = when {
            b < 0x80 -> 0
            b shr 5 == 0x6 -> 1
            b shr 4 == 0xe -> 2
            b shr 3 == 0x1e -> 3
            else -> return null
        }
        if (i + extra > size - 1) return null
        for (j in 1..extra) {
            if ((this[i + j].toInt() and 0xc0) != 0x80) return null
        }
        i += extra + 1
    }
    return decodeToString()
}

/** A message's fields in the order the wire has them, or null when it isn't a message. */
fun protoFields(bytes: ByteArray): List<ProtoField>? {
    val reader = ProtoReader(bytes)
    val fields = ArrayList<ProtoField>()
    while (reader.at < bytes.size) {
        val field = reader.field() ?: return null
        if (field.wire == 4) return null
        fields.add(ProtoField(field.number, field.value))
    }
    return fields
}

/** The last value of a field that isn't repeated, as a parser keeps it. */
fun List<ProtoField>.last(number: Int): ProtoField? = lastOrNull { it.number == number }

private class ProtoReader(private val bytes: ByteArray) {
    var at = 0

    fun varint(): ULong? {
        var value = 0UL
        var shift = 0
        while (at < bytes.size && shift < 64) {
            val byte = bytes[at].toInt()
            at++
            value = value or ((byte and 0x7f).toULong() shl shift)
            if (byte and 0x80 == 0) return value
            shift += 7
        }
        return null
    }

    private fun fixed(count: Int): ULong? {
        if (bytes.size - at < count) return null
        var value = 0UL
        for (i in 0 until count) {
            value = value or (bytes[at + i].toULong() and 0xffu shl (8 * i))
        }
        at += count
        return value
    }

    /** The next field. An end-group tag comes back with wire type 4 and no value of its own. */
    fun field(depth: Int = 0): Field? {
        val tag = varint() ?: return null
        if (tag shr 3 == 0UL || tag shr 3 > 536_870_911UL) return null
        val number = (tag shr 3).toInt()
        val wire = (tag and 7u).toInt()
        when (wire) {
            0 -> return Field(number, wire, WireValue.Varint(varint() ?: return null))
            1 -> return Field(number, wire, WireValue.Fixed64(fixed(8) ?: return null))
            2 -> {
                val length = varint() ?: return null
                if (length > (bytes.size - at).toULong()) return null
                val start = at
                at += length.toInt()
                return Field(number, wire, WireValue.Bytes(bytes.copyOfRange(start, at)))
            }
            3 -> {
                if (depth >= 32) return null
                val start = at
                while (true) {
                    val end = at
                    val inner = field(depth + 1) ?: return null
                    if (inner.wire == 4) {
                        if (inner.number != number) return null
                        return Field(number, wire, WireValue.Group(bytes.copyOfRange(start, end)))
                    }
                }
            }
            4 -> return Field(number, wire, WireValue.Group(ByteArray(0)))
            5 -> return Field(
                number, wire,
                WireValue.Fixed32((fixed(4) ?: return null).toUInt()))
            else -> return null
        }
    }

    class Field(val number: Int, val wire: Int, val value: WireValue)
}

class ProtoWriter {
    val bytes = ArrayList<Byte>()

    private fun raw(value: ULong) {
        var v = value
        while (v >= 0x80u) {
            bytes.add(((v and 0x7fu).toInt() or 0x80).toByte())
            v = v shr 7
        }
        bytes.add(v.toByte())
    }

    fun varint(field: Int, value: ULong) {
        raw(field.toULong() shl 3)
        raw(value)
    }

    /** An int32, int64 or enum: a negative one is its 64-bit two's complement. */
    fun int(field: Int, value: Long) = varint(field, value.toULong())

    fun bool(field: Int, value: Boolean) = varint(field, if (value) 1uL else 0uL)

    fun bytes(field: Int, value: ByteArray) {
        raw(field.toULong() shl 3 or 2u)
        raw(value.size.toULong())
        for (b in value) bytes.add(b)
    }

    fun string(field: Int, value: String) = bytes(field, value.encodeToByteArray())

    fun message(field: Int, build: ProtoWriter.() -> Unit) {
        val inner = ProtoWriter()
        inner.build()
        bytes(field, inner.toByteArray())
    }

    /** A field as it was read, whatever it is: what a message is written back with when only
     * some of its fields are this ilo's to change. */
    fun field(field: ProtoField) {
        val number = field.number.toULong() shl 3
        when (val value = field.value) {
            is WireValue.Varint -> varint(field.number, value.value)
            is WireValue.Fixed64 -> {
                raw(number or 1u)
                for (shift in 0 until 64 step 8) {
                    bytes.add((value.value shr shift).toByte())
                }
            }
            is WireValue.Bytes -> bytes(field.number, value.value)
            is WireValue.Group -> {
                raw(number or 3u)
                for (b in value.value) bytes.add(b)
                raw(number or 4u)
            }
            is WireValue.Fixed32 -> {
                raw(number or 5u)
                for (shift in 0 until 32 step 8) {
                    bytes.add((value.value shr shift).toByte())
                }
            }
        }
    }

    fun toByteArray(): ByteArray = ByteArray(bytes.size) { bytes[it] }
}