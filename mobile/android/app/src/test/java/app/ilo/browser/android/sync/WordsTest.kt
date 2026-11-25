package app.ilo.browser.android.sync

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test

class WordsTest {

    @Test
    fun wordListIsBip39() {
        // 2048 words, sorted, each unique: the BIP-39 English list Chromium's core/sync_words.cc uses.
        assertEquals(2048, syncWords.size)
        assertEquals(syncWords.sorted(), syncWords)
        assertEquals(2048, syncWords.toSet().size)
        assertEquals("abandon", syncWords[0])
        assertEquals("zoo", syncWords.last())
    }

    @Test
    fun roundTrip() {
        val secret = ByteArray(RecoveryWords.SECRET_LENGTH) { (it * 11 + 3).toByte() }
        val words = RecoveryWords.words(secret)
        assertEquals(24, words.split(" ").size)
        assertArrayEquals(secret, RecoveryWords.secret(words))
    }

    @Test
    fun acceptsAnyCaseAndSpacing() {
        val secret = ByteArray(RecoveryWords.SECRET_LENGTH) { 5 }
        val words = RecoveryWords.words(secret)
        assertArrayEquals(secret, RecoveryWords.secret(words.uppercase()))
        assertArrayEquals(secret, RecoveryWords.secret("  " + words.replace(" ", "  ") + "\n"))
    }

    @Test
    fun rejectsWrongChecksum() {
        val secret = ByteArray(RecoveryWords.SECRET_LENGTH) { 5 }
        val words = RecoveryWords.words(secret).split(" ").toMutableList()
        // Swap one word for its neighbor: a valid word, but not the one the checksum wants.
        words[7] = syncWords[syncWords.indexOf(words[7]) + 1]
        assertNull(RecoveryWords.secret(words.joinToString(" ")))
    }

    @Test
    fun rejectsUnknownAndShortInput() {
        assertNull(RecoveryWords.secret("not a real list of twenty four words at all here"))
        assertNull(RecoveryWords.secret("abandon " + RecoveryWords.words(ByteArray(32) { 0 })))
    }

    @Test
    fun prefixLookup() {
        val matches = RecoveryWords.startingWith("abando")
        assertEquals(listOf("abandon"), matches)
        // The list has mountain and mouse, and no word "mou" itself.
        assertEquals(listOf("mountain", "mouse"), RecoveryWords.startingWith("mou"))
        assertEquals(emptyList<String>(), RecoveryWords.startingWith(""))
        assertEquals(emptyList<String>(), RecoveryWords.startingWith("qqq"))
    }

    @Test
    fun knownVector() {
        // A zero secret: its checksum byte is the first of SHA-256 of 32 zeros, all readable
        // from the list alone, so the words are checked against the algorithm, not the code.
        val words = RecoveryWords.words(ByteArray(RecoveryWords.SECRET_LENGTH))
        assertNotNull(RecoveryWords.secret(words))
        assertArrayEquals(ByteArray(RecoveryWords.SECRET_LENGTH), RecoveryWords.secret(words))
    }
}