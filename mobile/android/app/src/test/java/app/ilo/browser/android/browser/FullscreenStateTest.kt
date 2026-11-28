package app.ilo.browser.android.browser

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The custom view's transitions: WebView's own calls are rough, so the state takes them. */
class FullscreenStateTest {
    @Test
    fun `show then hide runs the exit once`() {
        val state = FullscreenState<String>()
        var exits = 0
        state.show("video") { exits++ }
        assertTrue(state.active)
        state.hide()?.second?.invoke()
        assertFalse(state.active)
        assertEquals(1, exits)
    }

    @Test
    fun `a second hide answers null and runs no second exit`() {
        val state = FullscreenState<String>()
        state.show("video") {}
        state.hide()
        assertNull(state.hide())
        assertFalse(state.active)
    }

    @Test
    fun `a hide with nothing up answers null`() {
        val state = FullscreenState<String>()
        assertNull(state.hide())
        assertFalse(state.active)
    }

    @Test
    fun `hiding the old view after a second show runs only the old exit`() {
        val state = FullscreenState<String>()
        var oldExits = 0
        var newExits = 0
        state.show("first") { oldExits++ }
        // The caller takes the old pair down before showing the second, as Chrome does.
        state.hide()?.second?.invoke()
        state.show("second") { newExits++ }
        state.hide()?.second?.invoke()
        assertEquals(1, oldExits)
        assertEquals(1, newExits)
    }
}