package app.ilo.browser.android.browser

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** What the Space does with an address it holds, and which tab a search made over a page is. */
class TabAtTest {
    private fun state() = BrowserState(createTempDir())

    @Test
    fun anAddressTheSpaceHoldsIsFound() {
        val state = state()
        val held = state.newTab("https://held.example/")
        assertEquals(held.id, state.tabAt("https://held.example/")?.id)
    }

    @Test
    fun anAddressNoTabHoldsIsNotFound() {
        val state = state()
        state.newTab("https://other.example/")
        assertNull(state.tabAt("https://held.example/"))
    }

    @Test
    fun aPrivateTabIsNeverTheAddress() {
        val state = state()
        state.newTab("https://held.example/", isPrivate = true)
        // A private tab is its own: an ordinary address never lands on it.
        assertNull(state.tabAt("https://held.example/"))
    }

    @Test
    fun anotherSpacesTabIsNotThisSpaces() {
        val state = state()
        val home = state.currentSpace.value
        val work = state.addSpace("Work")
        state.newTab("https://held.example/", space = work.id)
        state.selectSpace(home)
        // The tab belongs to Work: the Space on screen doesn't hold its address.
        assertNull(state.tabAt("https://held.example/"))
    }
}

private fun createTempDir(): File = kotlin.io.path.createTempDirectory("ilo-test").toFile()