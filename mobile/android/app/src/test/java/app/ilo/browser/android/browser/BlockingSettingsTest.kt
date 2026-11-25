package app.ilo.browser.android.browser

import app.ilo.browser.android.blocking.EasyListBlocker
import app.ilo.browser.android.blocking.FakePrefs
import app.ilo.browser.android.blocking.FilterParser
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** The settings facade writes the engine's own store: what it writes, the engine blocks by. */
class BlockingSettingsTest {

    private fun engine(): EasyListBlocker =
        EasyListBlocker(FakePrefs()).apply {
            replaceLists(listOf(FilterParser.parse("test", "||ads.example.com^")))
        }

    @Test
    fun offSwitchReachesTheEngine() {
        val settings = BlockingSettings(engine(), CoroutineScope(SupervisorJob()))
        assertTrue(settings.isOn)
        settings.toggle(false)
        assertFalse(settings.isOn)
    }

    @Test
    fun siteExceptionReachesTheEngine() {
        val blocker = engine()
        val settings = BlockingSettings(blocker, CoroutineScope(SupervisorJob()))
        assertTrue(blocker.blocks("https://ads.example.com/x.js", "https://news.org/"))
        settings.setSite("news.org", false)
        assertEquals(listOf("news.org"), settings.sitesOff())
        assertFalse(blocker.blocks("https://ads.example.com/x.js", "https://news.org/"))
        settings.setSite("news.org", true)
        assertTrue(settings.sitesOff().isEmpty())
        assertTrue(blocker.blocks("https://ads.example.com/x.js", "https://news.org/"))
    }

    @Test
    fun theSitesOffFlowHearsAChange() {
        val settings = BlockingSettings(engine(), CoroutineScope(SupervisorJob()))
        assertEquals(emptyList<String>(), settings.sitesOffFlow.value)
        settings.setSite("news.org", false)
        assertEquals(listOf("news.org"), settings.sitesOffFlow.value)
        settings.setSite("news.org", true)
        assertEquals(emptyList<String>(), settings.sitesOffFlow.value)
    }

    @Test
    fun siteOffAnswersForThePagesUrl() {
        val settings = BlockingSettings(engine(), CoroutineScope(SupervisorJob()))
        assertFalse(settings.siteOff("https://news.org/story"))
        settings.setSite("news.org", false)
        assertTrue(settings.siteOff("https://news.org/story"))
    }
}