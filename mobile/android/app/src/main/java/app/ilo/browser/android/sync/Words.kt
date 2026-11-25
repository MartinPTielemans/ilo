// The account's secret as 24 words, as core/sync_words.cc writes and reads them: the secret's
// 256 bits and 8 of checksum, 11 bits a word.

package app.ilo.browser.android.sync

object RecoveryWords {
    const val SECRET_LENGTH = 32

    private fun withChecksum(secret: ByteArray): ByteArray = secret + Crypto.sha256(secret)[0]

    /** The words in lowercase with one space between, which is also the account's passphrase. */
    fun words(secret: ByteArray): String {
        require(secret.size == SECRET_LENGTH)
        val bits = withChecksum(secret)
        return (0 until 24).joinToString(" ") { word ->
            var index = 0
            for (bit in word * 11 until word * 11 + 11) {
                index = index shl 1 or (bits[bit / 8].toInt() ushr (7 - bit % 8) and 1)
            }
            syncWords[index]
        }
    }

    /** The secret that words typed in any case, with any space around them, stand for. */
    fun secret(words: String): ByteArray? {
        val typed = words.lowercase().split(Regex("\\s+")).filter { it.isNotEmpty() }
        if (typed.size != 24) return null
        val bits = ByteArray(SECRET_LENGTH + 1)
        var at = 0
        for (word in typed) {
            val index = indexOf(word) ?: return null
            for (bit in 10 downTo 0) {
                bits[at / 8] = (bits[at / 8].toInt() or
                    ((index shr bit and 1) shl (7 - at % 8))).toByte()
                at++
            }
        }
        val secret = bits.copyOfRange(0, SECRET_LENGTH)
        return if (withChecksum(secret)[SECRET_LENGTH] == bits[SECRET_LENGTH]) secret else null
    }

    /** The list is sorted, so a word and the words after a prefix are found by halving. */
    private fun lowerBound(text: String): Int {
        var low = 0
        var high = syncWords.size
        while (low < high) {
            val middle = (low + high) / 2
            if (syncWords[middle] < text) low = middle + 1 else high = middle
        }
        return low
    }

    private fun indexOf(word: String): Int? {
        val at = lowerBound(word)
        return if (at < syncWords.size && syncWords[at] == word) at else null
    }

    fun startingWith(prefix: String): List<String> {
        val lower = prefix.lowercase()
        if (lower.isEmpty()) return emptyList()
        val at = lowerBound(lower)
        val out = ArrayList<String>()
        var i = at
        while (i < syncWords.size && syncWords[i].startsWith(lower)) {
            out.add(syncWords[i])
            i++
        }
        return out
    }
}