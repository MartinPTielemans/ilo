package app.ilo.browser.android.ui

import org.junit.Assert.assertEquals
import org.junit.Test

class AccountScreenTest {

    @Test
    fun suggestionsForAPartialWord() {
        assertEquals(
            listOf("abandon"),
            recoveryWordSuggestions("abando"))
    }

    @Test
    fun noSuggestionForAFinishedWord() {
        // A trailing space says the word is done, so nothing is offered for the next one yet.
        assertEquals(emptyList<String>(), recoveryWordSuggestions("abandon "))
    }

    @Test
    fun suggestionsAfterEarlierWords() {
        assertEquals(
            listOf("abandon"),
            recoveryWordSuggestions("zoo result abando"))
    }

    @Test
    fun nothingForAWordNotOnTheList() {
        assertEquals(emptyList<String>(), recoveryWordSuggestions("qqqq"))
    }

    @Test
    fun nothingPastTwentyFourWords() {
        val twentyFour = (List(24) { "abandon" }).joinToString(" ")
        assertEquals(emptyList<String>(), recoveryWordSuggestions("$twentyFour abando"))
    }

    @Test
    fun nothingForEmptyText() {
        assertEquals(emptyList<String>(), recoveryWordSuggestions(""))
    }
}