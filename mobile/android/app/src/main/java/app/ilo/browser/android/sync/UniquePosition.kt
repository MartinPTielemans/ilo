// A port of Chromium's components/sync/base/unique_position.cc, at 149.0.7827.4: where a row
// sits among its siblings, as bytes that order as the rows do. The computer orders the sidebar
// by these and makes one for each row it adds, so the phone compares and makes them the same way.

package app.ilo.browser.android.sync

class UniquePosition private constructor(var compressed: ByteArray) {

    val isValid: Boolean get() = compressed.isNotEmpty()

    constructor() : this(ByteArray(0))

    private fun parse(proto: ByteArray) {
        val fields = protoFields(proto) ?: return
        val custom = fields.last(4)?.bytes
        if (custom != null) {
            compressed = if (isValidBytes(uncompress(custom))) custom else ByteArray(0)
            return
        }
        val value = fields.last(1)?.bytes
        if (value != null && value.isNotEmpty()) {
            compressed = compress(value)
            return
        }
        // The third form, zlib-compressed, is one Chromium no longer writes; a server's answer
        // could still have one, and this port doesn't carry a zlib inflate for it, so it reads
        // as invalid rather than crashing.
    }

    /** The form Chromium writes now. */
    fun proto(): ByteArray {
        val writer = ProtoWriter()
        writer.bytes(4, compressed)
        return writer.toByteArray()
    }

    fun lessThan(other: UniquePosition): Boolean = less(compressed, other.compressed)

    override fun equals(other: Any?): Boolean =
        other is UniquePosition && other.compressed.contentEquals(compressed)

    override fun hashCode(): Int = compressed.contentHashCode()

    companion object {
        /** From a serialized sync_pb.UniquePosition, in any of the three forms it has had. */
        fun fromProto(proto: ByteArray): UniquePosition {
            val position = UniquePosition()
            position.parse(proto)
            return position
        }

        private fun ofCompressed(bytes: ByteArray): UniquePosition {
            val position = UniquePosition(ByteArray(0))
            position.compressed = bytes
            return position
        }
        const val SUFFIX_LENGTH = 28
        private const val MAX_UNCOMPRESSED = 10 * 1024 * 1024

        /** What makes a position unlike every other: random bytes, the last never zero. */
        fun randomSuffix(): ByteArray {
            val suffix = Crypto.randomBytes(SUFFIX_LENGTH)
            suffix[SUFFIX_LENGTH - 1] = 0x7f
            return suffix
        }

        fun initial(suffix: ByteArray): UniquePosition = ofCompressed(compress(suffix))

        /** Before and after a position that isn't valid there is nothing to find, so those give
         * a first position, where Chromium only asserts. */
        fun before(x: UniquePosition, suffix: ByteArray): UniquePosition {
            if (!x.isValid) return initial(suffix)
            return ofCompressed(compress(findSmaller(uncompress(x.compressed), suffix) + suffix))
        }

        fun after(x: UniquePosition, suffix: ByteArray): UniquePosition {
            if (!x.isValid) return initial(suffix)
            return ofCompressed(compress(findGreater(uncompress(x.compressed), suffix) + suffix))
        }

        fun between(before: UniquePosition, after: UniquePosition, suffix: ByteArray): UniquePosition =
            ofCompressed(
                compress(
                    findBetween(
                        uncompress(before.compressed), uncompress(after.compressed), suffix) + suffix))

        /** Bytes compared as std::string compares them: by unsigned value, a prefix first. */
        private fun less(a: ByteArray, b: ByteArray): Boolean {
            val n = minOf(a.size, b.size)
            for (i in 0 until n) {
                val x = a[i].toInt() and 0xff
                val y = b[i].toInt() and 0xff
                if (x != y) return x < y
            }
            return a.size < b.size
        }

        private fun isValidBytes(bytes: ByteArray): Boolean =
            bytes.size >= SUFFIX_LENGTH && bytes[bytes.size - 1] != 0.toByte()

        // The shortest prefix that, with `suffix` after it, sorts before `reference`.
        private fun findSmaller(reference: ByteArray, suffix: ByteArray): ByteArray {
            val refZeroes = reference.indexOfFirst { it != 0.toByte() }.let {
                if (it == -1) reference.size else it
            }
            val suffixZeroes = suffix.indexOfFirst { it != 0.toByte() }.let {
                if (it == -1) suffix.size else it
            }
            if (suffixZeroes > refZeroes) {
                // Implies suffix < reference.
                return ByteArray(0)
            }
            if (less(suffix.copyOfRange(suffixZeroes, suffix.size),
                    reference.copyOfRange(refZeroes, reference.size))
            ) {
                // Prepend zeroes so the result has as many zero digits as `reference`.
                return ByteArray(refZeroes - suffixZeroes)
            } else if (suffixZeroes > 1) {
                // Prepend zeroes so the result has one more zero digit than `reference`.
                return ByteArray(refZeroes - suffixZeroes + 1)
            } else {
                // Prepend zeroes to match those in `reference`, then a digit smaller than its
                // first non-zero one.
                return ByteArray(refZeroes) + byteArrayOf((reference[refZeroes].toInt() / 2).toByte())
            }
        }

        // The shortest prefix that, with `suffix` after it, sorts after `reference`.
        private fun findGreater(reference: ByteArray, suffix: ByteArray): ByteArray {
            val refFFs = reference.indexOfFirst { it != 0xff.toByte() }.let {
                if (it == -1) reference.size else it
            }
            val suffixFFs = suffix.indexOfFirst { it != 0xff.toByte() }.let {
                if (it == -1) suffix.size else it
            }
            if (suffixFFs > refFFs) {
                // Implies suffix > reference.
                return ByteArray(0)
            }
            if (less(reference.copyOfRange(refFFs, reference.size),
                    suffix.copyOfRange(suffixFFs, suffix.size))
            ) {
                return ByteArray(refFFs - suffixFFs) { 0xff.toByte() }
            } else if (suffixFFs > 1) {
                return ByteArray(refFFs - suffixFFs + 1) { 0xff.toByte() }
            } else {
                // A digit halfway between the reference's first non-0xff one and 0xff. Chromium
                // reads the string's terminator when the reference is all 0xff, which a valid
                // position isn't.
                val digit = if (refFFs < reference.size) reference[refFFs].toInt() and 0xff else 0
                return ByteArray(refFFs) { 0xff.toByte() } +
                    byteArrayOf((digit + (255 - digit + 1) / 2).toByte())
            }
        }

        private fun findBetween(before: ByteArray, after: ByteArray, suffix: ByteArray): ByteArray {
            val middle = ArrayList<Byte>()
            // The suffix alone may already sort between the two.
            if (less(before, suffix) && less(suffix, after)) return ByteArray(0)
            var i = 0
            while (i < minOf(before.size, after.size)) {
                val a = before[i].toInt() and 0xff
                val b = after[i].toInt() and 0xff
                if (b - a >= 2) {
                    middle.add(((a + (b - a) / 2)).toByte())
                    return middle.toByteArray()
                } else if (a == b) {
                    middle.add(a.toByte())
                    // The suffix may sort between the two from here on.
                    if (less(before.copyOfRange(i + 1, before.size), suffix) &&
                        less(suffix, after.copyOfRange(i + 1, after.size))
                    ) {
                        return middle.toByteArray()
                    }
                } else {
                    // The digits differ by one: take the shorter of going just above `before`
                    // under its digit and just below `after` under its own.
                    val above = middle.toByteArray() + byteArrayOf(a.toByte()) +
                        findGreater(before.copyOfRange(i + 1, before.size), suffix)
                    if (after.size > i + 1) {
                        val below = middle.toByteArray() + byteArrayOf(b.toByte()) +
                            findSmaller(after.copyOfRange(i + 1, after.size), suffix)
                        if (below.size < above.size) return below
                    }
                    return above
                }
                i++
            }
            // `before` is a prefix of `after`, so anything smaller than what's left of `after`
            // does.
            return middle.toByteArray() + findSmaller(after.copyOfRange(i, after.size), suffix)
        }

        private fun isRepeated(bytes: ByteArray, at: Int): Boolean =
            bytes[at] == bytes[at + 1] && bytes[at] == bytes[at + 2] && bytes[at] == bytes[at + 3]

        // Blocks of eight bytes: four of one digit and a count stand for a run of it, the count
        // written from the top down when the digit after the run is greater, so that order is kept.
        private fun compress(bytes: ByteArray): ByteArray {
            val out = ArrayList<Byte>(48)
            var i = 0
            while (i < bytes.size) {
                if (i + 4 <= bytes.size && isRepeated(bytes, i)) {
                    for (k in 0 until 4) out.add(bytes[i + k])
                    val digit = bytes[i]
                    val end = (i + 4 until bytes.size).firstOrNull { bytes[it] != digit }
                    val run = (end ?: bytes.size) - i
                    val high = end != null && bytes[end] > digit
                    val encoded = if (high) 0xffff_ffffu - run.toUInt() else run.toUInt()
                    out.add((encoded shr 24).toByte())
                    out.add((encoded shr 16 and 0xffu).toByte())
                    out.add((encoded shr 8 and 0xffu).toByte())
                    out.add((encoded and 0xffu).toByte())
                    i += run
                } else {
                    val count = minOf(8, bytes.size - i)
                    for (k in 0 until count) out.add(bytes[i + k])
                    i += count
                }
            }
            return out.toByteArray()
        }

        private fun uncompress(bytes: ByteArray): ByteArray {
            if (bytes.size > MAX_UNCOMPRESSED) return ByteArray(0)
            val out = ArrayList<Byte>(bytes.size)
            var i = 0
            while (i + 8 <= bytes.size) {
                if (isRepeated(bytes, i)) {
                    val encoded = (bytes[i + 4].toUInt() and 0xffu) shl 24 or
                        ((bytes[i + 5].toUInt() and 0xffu) shl 16) or
                        ((bytes[i + 6].toUInt() and 0xffu) shl 8) or
                        (bytes[i + 7].toUInt() and 0xffu)
                    val run = (if (encoded and 0x8000_0000u != 0u) 0xffff_ffffu - encoded
                        else encoded).toInt()
                    if (out.size + run > MAX_UNCOMPRESSED) return ByteArray(0)
                    repeat(run) { out.add(bytes[i]) }
                } else {
                    for (k in 0 until 8) out.add(bytes[i + k])
                }
                i += 8
            }
            while (i < bytes.size) {
                out.add(bytes[i])
                i++
            }
            val result = out.toByteArray()
            return if (result.size > MAX_UNCOMPRESSED) ByteArray(0) else result
        }
    }
}