package app.ilo.browser.android.browser

import app.ilo.browser.android.blocking.FakePrefs
import app.ilo.browser.android.ui.Metrics
import app.ilo.browser.android.ui.cornersInside
import app.ilo.browser.android.ui.fanGeometry
import java.io.File
import kotlin.math.min
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The contracts the batch's UI sits on: custom engines, a tab's own dark, and history removal. */
class BatchContractsTest {
    private fun settings() = BrowserSettings(FakePrefs())

    @Test
    fun aCustomEngineSitsBesideTheShippedOnes() {
        val settings = settings()
        settings.customEngines = listOf(SearchEngine("wikipedia", "Wikipedia", "https://en.wikipedia.org/w/index.php?search="))
        assertTrue(settings.allEngines().any { it.id == "duckduckgo" })
        assertEquals("Wikipedia", settings.engineNamed("wikipedia").name)
    }

    @Test
    fun customEnginesSurviveTheirOwnStore() {
        val prefs = FakePrefs()
        val first = BrowserSettings(prefs)
        first.customEngines = listOf(SearchEngine("wikipedia", "Wikipedia", "https://en.wikipedia.org/w/index.php?search="))
        val second = BrowserSettings(prefs)
        assertEquals(1, second.customEngines.size)
        assertEquals("wikipedia", second.customEngines.first().id)
    }

    @Test
    fun aBrokenStoredEngineListIsNothing() {
        val prefs = FakePrefs()
        prefs.edit().putString("customEngines", "not a list").apply()
        val settings = BrowserSettings(prefs)
        assertTrue(settings.customEngines.isEmpty())
    }

    @Test
    fun deletingTheChosenEngineFallsBackToTheStandardOne() {
        val settings = settings()
        settings.customEngines = listOf(SearchEngine("mine", "Mine", "https://mine.example/search?q="))
        settings.searchEngine = settings.engineNamed("mine")
        assertEquals("mine", settings.searchEngine.id)
        settings.customEngines = emptyList()
        assertEquals(SearchEngine.standard.id, settings.searchEngine.id)
    }

    @Test
    fun resolvedDarkFollowsTheSettingThenTheSystem() {
        val settings = settings()
        settings.darkMode = "light"
        assertFalse(settings.resolvedDark(systemDark = true))
        settings.darkMode = "dark"
        assertTrue(settings.resolvedDark(systemDark = false))
        settings.darkMode = "system"
        assertTrue(settings.resolvedDark(systemDark = true))
        assertFalse(settings.resolvedDark(systemDark = false))
    }

    @Test
    fun pageDarkStartsAtFollowAndKeepsWhatWasChosen() {
        val settings = settings()
        assertEquals("follow", settings.pageDark)
        settings.pageDark = "darken"
        assertEquals("darken", settings.pageDark)
        settings.pageDark = "off"
        assertEquals("off", settings.pageDark)
        settings.pageDark = "nonsense"
        assertEquals("follow", settings.pageDark)
    }

    @Test
    fun aTabsOwnDarkSurvivesTheSession() {
        val dir = createTempDir()
        val first = BrowserState(dir)
        val tab = first.newTab("https://example.com")
        first.update(tab.copy(contentDark = true))

        val second = BrowserState(dir)
        assertEquals(true, second.tabs.value.first().contentDark)
    }

    @Test
    fun aTabWithNoChoiceHasNoDarkOfItsOwn() {
        val state = BrowserState(createTempDir())
        val tab = state.newTab("https://example.com")
        assertNull(tab.contentDark)
    }

    @Test
    fun theStartPageAddressIsTheSitesOwnDomain() {
        val settings = settings()
        assertEquals("https://ilobrowser.com", settings.startPageUrl)
        settings.startPageUrl = "https://example.com/"
        assertEquals("https://example.com/", settings.startPageUrl)
    }

    @Test
    fun removingOneHistoryEntryLeavesTheRest() {
        val stores = PersonalStores(createTempDir())
        stores.visit("https://example.com", "Example")
        stores.visit("https://ilo.page", "ilo")
        stores.removeHistory("https://example.com")
        assertEquals(1, stores.historyFlow.value.size)
        assertEquals("https://ilo.page", stores.historyFlow.value.first().url)
    }

    @Test
    fun anArchivedTabKeepsItsOwnDark() {
        val stores = PersonalStores(createTempDir())
        val state = BrowserState(createTempDir())
        state.newTab("https://example.com")
        val dark = state.tabs.value.first().copy(contentDark = true)
        state.update(dark)
        stores.archive(dark)
        val archived = stores.archivedFlow.value.first()
        assertEquals(true, archived.contentDark)
        stores.unarchive(archived.id)
        assertTrue(stores.archivedFlow.value.isEmpty())
    }

    private fun createTempDir(): File = kotlin.io.path.createTempDirectory("ilo-test").toFile()

    @Test
    fun switcherDialsOpenAtTheOwnersNumbers() {
        val settings = settings()
        assertEquals(0.75f, settings.switcherCardShare)
        assertEquals(0.70f, settings.switcherOverlap)
    }

    @Test
    fun switcherDialsClampToTheirBounds() {
        val settings = settings()
        settings.switcherCardShare = 1.4f
        assertEquals(0.95f, settings.switcherCardShare)
        settings.switcherCardShare = 0.01f
        assertEquals(0.10f, settings.switcherCardShare)
        settings.switcherOverlap = 1.4f
        assertEquals(0.95f, settings.switcherOverlap)
        settings.switcherOverlap = 0.01f
        assertEquals(0.10f, settings.switcherOverlap)
    }

    @Test
    fun switcherDialsSurviveTheirOwnStore() {
        val prefs = FakePrefs()
        val first = BrowserSettings(prefs)
        first.switcherCardShare = 0.35f
        first.switcherOverlap = 0.65f
        val second = BrowserSettings(prefs)
        assertEquals(0.35f, second.switcherCardShare)
        assertEquals(0.65f, second.switcherOverlap)
    }

    /** The dial bounds were picked so the fan's shape fits a phone-sized deck at every
     * corner; fanGeometry is the same math the deck draws, so its whole range is checked
     * straight against itself, alone and with two behind cards wanting in. */
    @Test
    fun everySwitcherDialCornerFitsTheDeck() {
        val corners = listOf(0.10f, 0.55f, 0.70f, 0.95f)
        for (share in corners) {
            for (covered in corners) {
                for (tabs in listOf(1, 2, 3, 6)) {
                    val g = fanGeometry(1080f, 2100f, share, covered, tabs)
                    assertTrue("share $share covered $covered tabs $tabs width ${g.cardWidth}", g.cardWidth <= 1080f)
                    assertTrue("share $share covered $covered tabs $tabs height ${g.cardHeight}", g.cardHeight <= 2100f)
                    assertTrue("share $share covered $covered tabs $tabs stair ${g.stair} shown ${g.shownBehind}",
                        g.cardHeight * (1f + Metrics.Switcher.stair * g.shownBehind) <= 2100f + 0.5f)
                    val frontLeft = 540f - g.fanShift - g.cardWidth / 2f
                    assertTrue("share $share covered $covered tabs $tabs front $frontLeft", frontLeft >= 0f)
                    assertTrue("share $share covered $covered tabs $tabs behind ${g.shownBehind} of ${min(2, tabs - 1)}",
                        g.shownBehind <= min(2, tabs - 1))
                    // Every shown behind card's rotated corners keep inside by its own tilt.
                    for (depth in 1..g.shownBehind) {
                        val center = 540f + g.step * depth - g.fanShift
                        assertTrue("share $share covered $covered tabs $tabs tilt depth $depth",
                            cornersInside(1080f, center, g.cardWidth, g.cardHeight, g.tilts[depth - 1]))
                    }
                }
            }
        }
    }
}