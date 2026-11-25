package app.ilo.browser.android.ui

import app.ilo.browser.android.browser.PageEngine
import androidx.compose.ui.graphics.toArgb
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** The page's own color on the bar, read from the page's theme-color meta as Chrome's bar does. */
class PageColorTest {

    @Test
    fun aPageNamedAColorTheBarTakes() {
        // A page naming red: the bar's wash is red over the plate, not the Space's green.
        val plain = Look.of(dark = false, spaceColor = null)
        val red = Look.of(dark = false, spaceColor = null, pageColor = 0xFF0000)
        assertNotEquals(plain.tinted.toArgb(), red.tinted.toArgb())
    }

    @Test
    fun aPageWithoutAColorKeepsTheSpacesTint() {
        // No pageColor and no Space color: the green accent washes the bar, as before.
        val none = Look.of(dark = false, spaceColor = null)
        val green = Look.of(dark = false, spaceColor = null, pageColor = null)
        assertEquals(none.tinted.toArgb(), green.tinted.toArgb())
    }

    @Test
    fun aPrivateBarKeepsItsInkWhateverThePageSaid() {
        // Private chrome never takes a page's color, as it takes no Space color.
        val private = Look.of(dark = false, spaceColor = null, isPrivate = true, pageColor = 0xFF0000)
        assertEquals(Look.Ink.toArgb(), private.tinted.toArgb())
    }

    @Test
    fun cssColorsParseToRgbLongs() {
        assertEquals(0xFF0000L, PageEngine.parseCssColor("\"#ff0000\""))
        assertEquals(0xFF0000L, PageEngine.parseCssColor("\"rgb(255, 0, 0)\""))
        assertEquals(0xFF0000L, PageEngine.parseCssColor("\"#f00\""))
        assertEquals(0x112233L, PageEngine.parseCssColor("\"#112233\""))
        assertEquals(0x334455L, PageEngine.parseCssColor("\"#88334455\""))
        assertEquals(0x57B97AL, PageEngine.parseCssColor("\"#57B97A\""))
        assertNull(PageEngine.parseCssColor("null"))
        assertNull(PageEngine.parseCssColor("\"not a color\""))
    }
}