package app.ilo.browser.android.browser

import java.io.File
import org.junit.Test
import org.junit.Assert.assertEquals

class StoreWriteTest {
    @Test
    fun bookmarkWritesTheFile() {
        val dir = kotlin.io.path.createTempDirectory("ilo-wt").toFile()
        val stores = PersonalStores(dir)
        stores.bookmark("https://example.com/", "Example")
        assertEquals(true, File(dir, "bookmarks.json").exists())
        assertEquals("https://example.com/", PersonalStores(dir).bookmarksFlow.value.first().url)
    }
}
