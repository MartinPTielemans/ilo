package app.ilo.browser.android.browser

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class AddressTest {
    private val engine = SearchEngine.standard

    @Test
    fun opensAnHttpAddress() {
        assertEquals("https://example.com", Address.asAddress("example.com"))
        assertEquals("https://example.com", Address.resolve("example.com", engine))
    }

    @Test
    fun keepsTheSchemeTheUserTyped() {
        assertEquals("http://example.com/path", Address.asAddress("http://example.com/path"))
    }

    @Test
    fun localHostsGoOverHttp() {
        assertEquals("http://localhost:8080", Address.asAddress("localhost:8080"))
        assertEquals("http://192.168.1.4", Address.asAddress("192.168.1.4"))
    }

    @Test
    fun aWordWithADotIsASite() {
        assertEquals("https://news.example.org/story", Address.asAddress("news.example.org/story"))
    }

    @Test
    fun anotherSchemeIsSearchedFor() {
        assertNull(Address.asAddress("mailto:someone@example.com"))
        assertEquals("https://duckduckgo.com/?q=mailto%3Asomeone%40example.com", Address.resolve("mailto:someone@example.com", engine))
    }

    @Test
    fun aFileEndingIsSearchedFor() {
        assertNull(Address.asAddress("notes.txt"))
    }

    @Test
    fun plainWordsSearch() {
        assertEquals("https://duckduckgo.com/?q=hello%20world", Address.resolve("hello world", engine))
    }

    @Test
    fun emptyTextOpensNothing() {
        assertNull(Address.resolve("   ", engine))
    }

    @Test
    fun spacesInAnAddressMakeItASearch() {
        assertNull(Address.asAddress("example.com and more"))
    }

    @Test
    fun aUrlWithoutAHostIsASearch() {
        assertNull(Address.asAddress("http://"))
    }

    @Test
    fun typedHttpsSchemeOpensDirectly() {
        assertEquals("https://example.com", Address.asAddress("https://example.com"))
        assertEquals("https://example.com", Address.resolve("https://example.com", engine))
    }
}
