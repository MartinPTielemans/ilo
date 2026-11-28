package app.ilo.browser.android.browser

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class FoldTest {
    private fun state() = BrowserState(createTempDir())

    @Test
    fun leavingToSpaceKeepsTheTabOpen() {
        val state = state()
        val tab = state.newTab("https://example.com")
        state.leaveToSpace()
        assertNull(state.selectedId.value)
        // The tab stays in its Space, its card ringed, nothing on screen.
        assertTrue(state.tabs.value.any { it.id == tab.id })
    }

    @Test
    fun leavingAndComingBackSelectsTheSameTab() {
        val state = state()
        val tab = state.newTab("https://example.com")
        state.leaveToSpace()
        state.select(tab.id)
        assertEquals(tab.id, state.selectedId.value)
    }

    @Test
    fun aPrivateTabOpenedWithNothingInIt() {
        val state = state()
        val tab = state.newTab(isPrivate = true)
        assertTrue(tab.isPrivate)
        assertNull(tab.url)
        assertEquals(tab.id, state.selectedId.value)
    }

    @Test
    fun barTuckedStartsUntucked() {
        val state = state()
        assertFalse(state.barTucked.value)
    }
}

private fun createTempDir(): File = kotlin.io.path.createTempDirectory("ilo-test").toFile()