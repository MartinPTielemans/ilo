package app.ilo.browser.android.blocking

import android.content.SharedPreferences
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.HashMap

/** Minimal in-memory SharedPreferences stand-in; values live only in the test. */
class FakePrefs : SharedPreferences {
    private val map = HashMap<String, Any>()

    override fun edit(): SharedPreferences.Editor = object : SharedPreferences.Editor {
        override fun putString(key: String, value: String?) = apply { if (value == null) map.remove(key) else map[key] = value }
        override fun putStringSet(key: String, values: MutableSet<String>?) = apply { if (values == null) map.remove(key) else map[key] = values.toSet() }
        override fun putInt(key: String, value: Int) = apply { map[key] = value }
        override fun putLong(key: String, value: Long) = apply { map[key] = value }
        override fun putFloat(key: String, value: Float) = apply { map[key] = value }
        override fun putBoolean(key: String, value: Boolean) = apply { map[key] = value }
        override fun remove(key: String) = apply { map.remove(key) }
        override fun clear() = apply { map.clear() }
        override fun commit() = true
        override fun apply() {}
    }

    override fun getAll() = map
    override fun getString(key: String, defValue: String?) = map[key] as? String ?: defValue
    override fun getStringSet(key: String, defValue: Set<String>?) = map[key] as? Set<String> ?: defValue
    override fun getInt(key: String, defValue: Int) = map[key] as? Int ?: defValue
    override fun getLong(key: String, defValue: Long) = map[key] as? Long ?: defValue
    override fun getFloat(key: String, defValue: Float) = map[key] as? Float ?: defValue
    override fun getBoolean(key: String, defValue: Boolean) = map[key] as? Boolean ?: defValue
    override fun contains(key: String) = map.containsKey(key)
    override fun registerOnSharedPreferenceChangeListener(l: SharedPreferences.OnSharedPreferenceChangeListener) {}
    override fun unregisterOnSharedPreferenceChangeListener(l: SharedPreferences.OnSharedPreferenceChangeListener) {}
}

class BlockerTest {

    private fun blockerWith(vararg lines: String): EasyListBlocker {
        val blocker = EasyListBlocker(FakePrefs())
        blocker.replaceLists(listOf(FilterParser.parse("test", lines.joinToString("\n"))))
        return blocker
    }

    @Test
    fun offSwitchDisablesBlocking() {
        val blocker = blockerWith("||ads.example.com^")
        assertTrue(blocker.blocks("https://ads.example.com/x.js", "https://news.org/"))
        blocker.setOn(false)
        assertFalse(blocker.blocks("https://ads.example.com/x.js", "https://news.org/"))
        assertFalse(blocker.isOn)
    }

    @Test
    fun aTabHeldOffBlocksNothing() {
        val blocker = blockerWith("||ads.example.com^")
        assertFalse(blocker.blocks("https://ads.example.com/x.js", "https://news.org/", heldChoice = false))
        // The master switch still wins over a held choice.
        assertTrue(blocker.blocks("https://ads.example.com/x.js", "https://news.org/", heldChoice = true))
    }

    @Test
    fun aTabHeldOnRunsOverTheSitesException() {
        val blocker = blockerWith("||ads.example.com^")
        blocker.setSiteBlockingOff("https://news.org/", off = true)
        assertFalse(blocker.blocks("https://ads.example.com/x.js", "https://news.org/"))
        assertTrue(blocker.blocks("https://ads.example.com/x.js", "https://news.org/", heldChoice = true))
        assertFalse(blocker.blocks("https://ads.example.com/x.js", "https://news.org/", heldChoice = null))
    }

    @Test
    fun domainAnchorBlocks() {
        val blocker = blockerWith("||doubleclick.net^")
        assertTrue(blocker.blocks("https://doubleclick.net/pixel.gif", "https://news.org/"))
        assertTrue(blocker.blocks("https://sub.doubleclick.net/pixel.gif", "https://news.org/"))
        assertFalse(blocker.blocks("https://notdoubleclick.net/pixel.gif", "https://news.org/"))
        assertFalse(blocker.blocks("https://example.com/doubleclick.net/", "https://news.org/"))
    }

    @Test
    fun plainSubstringBlocks() {
        val blocker = blockerWith("/adframe.")
        assertTrue(blocker.blocks("https://example.com/adframe.js", "https://news.org/"))
        assertFalse(blocker.blocks("https://example.com/frame.js", "https://news.org/"))
    }

    @Test
    fun thirdPartyOption() {
        val blocker = blockerWith("||tracker.example^\$third-party")
        assertFalse(blocker.blocks("https://tracker.example/a.js", "https://tracker.example/page"))
        assertTrue(blocker.blocks("https://tracker.example/a.js", "https://news.org/page"))
        val firstPartyOnly = blockerWith("||tracker.example^$~third-party")
        assertTrue(firstPartyOnly.blocks("https://tracker.example/a.js", "https://sub.tracker.example/page"))
        assertFalse(firstPartyOnly.blocks("https://tracker.example/a.js", "https://news.org/page"))
    }

    @Test
    fun domainOptionUsesPageOrigin() {
        val blocker = blockerWith("||ads.example.com^\$domain=news.org")
        assertTrue(blocker.blocks("https://ads.example.com/a.js", "https://news.org/page"))
        assertFalse(blocker.blocks("https://ads.example.com/a.js", "https://other.org/page"))
        val excluded = blockerWith("||ads.example.com^\$domain=~news.org")
        assertFalse(excluded.blocks("https://ads.example.com/a.js", "https://news.org/page"))
        assertTrue(excluded.blocks("https://ads.example.com/a.js", "https://other.org/page"))
    }

    @Test
    fun exceptionRuleWins() {
        val blocker = blockerWith("||example.com^", "@@||example.com/allowed/")
        assertFalse(blocker.blocks("https://example.com/allowed/a.js", "https://news.org/"))
        assertTrue(blocker.blocks("https://example.com/blocked.js", "https://news.org/"))
    }

    @Test
    fun lastRuleAndPerPageCounter() {
        val blocker = blockerWith("||ads.example.com^", "||tracker.example^")
        val page = "https://news.org/a"
        blocker.blocks("https://ads.example.com/1.js", page)
        blocker.blocks("https://tracker.example/2.js", page)
        assertEquals(2, blocker.blockedCount)
        assertEquals("||tracker.example^", blocker.lastRule)
        blocker.blocks("https://ads.example.com/3.js", "https://news.org/b")
        assertEquals(1, blocker.blockedCount)
        assertEquals("||ads.example.com^", blocker.lastRule)
    }

    @Test
    fun noMatchLeavesLastRuleNull() {
        val blocker = blockerWith("||ads.example.com^")
        blocker.blocks("https://clean.org/x.js", "https://news.org/")
        assertNull(blocker.lastRule)
        assertEquals(0, blocker.blockedCount)
    }

    @Test
    fun perSiteOffSwitch() {
        val blocker = blockerWith("||ads.example.com^")
        blocker.setSiteBlockingOff("https://news.org/", true)
        assertTrue(blocker.siteBlockingOff("https://news.org/article"))
        assertFalse(blocker.blocks("https://ads.example.com/a.js", "https://news.org/article"))
        assertFalse(blocker.blocks("https://ads.example.com/a.js", "https://sub.news.org/article"))
        blocker.setSiteBlockingOff("https://news.org/", false)
        assertTrue(blocker.blocks("https://ads.example.com/a.js", "https://news.org/article"))
    }

    @Test
    fun customListParseAndUse() {
        val blocker = EasyListBlocker(FakePrefs())
        blocker.replaceLists(listOf(FilterParser.parse("custom", "||customads.net^\n/example/ads/")))
        assertTrue(blocker.blocks("https://customads.net/banner.png", "https://news.org/"))
        assertTrue(blocker.blocks("https://cdn.example.com/example/ads/x.js", "https://news.org/"))
        assertFalse(blocker.blocks("https://clean.org/", "https://news.org/"))
    }

    @Test
    fun cosmeticRulesDoNotBlockNetwork() {
        val blocker = blockerWith("example.com##.advert")
        assertFalse(blocker.blocks("https://example.com/page.html", "https://example.com/page.html"))
    }

    @Test
    fun mixedCaseUrlBlocksAsLowered() {
        // A request's host can arrive in any case, as browsers send it; the rules are lower.
        val blocker = blockerWith("||ads.example.com^")
        assertTrue(blocker.blocks("https://ADS.Example.COM/x.js", "https://news.org/"))
        assertTrue(blocker.blocks("https://ads.example.com/PATH/X.js", "https://News.Org/"))
    }

    @Test
    fun siteExceptionWithMixedCasePageStillStops() {
        val blocker = blockerWith("||ads.example.com^")
        blocker.setSiteBlockingOff("https://news.org/", true)
        assertFalse(blocker.blocks("https://ads.example.com/a.js", "https://NEWS.org/article"))
    }

    @Test
    fun deepSubdomainWalksEveryLabel() {
        // The bucket a rule lands in is keyed by each label of its host, and a request whose
        // host carries more labels must still find it through the walk.
        val blocker = blockerWith("||ads.example.com^")
        assertTrue(blocker.blocks("https://a.b.c.ads.example.com/x.js", "https://news.org/"))
    }

    @Test
    fun pageHostCacheAnswersAcrossRequests() {
        val blocker = blockerWith("||ads.example.com^\$third-party")
        val page = "https://deep.news.org/article"
        // Many requests on one page: the cached host serves each without re-parsing.
        repeat(5) { assertTrue(blocker.blocks("https://ads.example.com/x$it.js", page)) }
        assertFalse(blocker.blocks("https://ads.example.com/x.js", "https://ads.example.com/page"))
    }

    @Test
    fun genericRuleMatchesWhenItsTokenIsInTheUrl() {
        // -adframe- is a generic rule: no anchored host, so it runs through the token index.
        val blocker = blockerWith("-adframe-")
        assertTrue(blocker.blocks("https://cdn.example.com/img-adframe-top.png", "https://news.org/"))
    }

    @Test
    fun genericRuleSkippedWhenItsTokenIsAbsent() {
        val blocker = blockerWith("-adframe-", "||ads.example.com^")
        // Nothing in the URL says adframe, so only the anchored rule answers.
        assertFalse(blocker.blocks("https://cdn.example.com/img-banner-top.png", "https://news.org/"))
    }

    @Test
    fun untokenedGenericRuleStillMatches() {
        // A pattern with no plain run before '^' can carry no token, so it is scanned always.
        val blocker = blockerWith("/-^")
        assertTrue(blocker.blocks("https://cdn.example.com/x/-/y.js", "https://news.org/"))
        assertFalse(blocker.blocks("https://cdn.example.com/x/y.js", "https://news.org/"))
    }

    @Test
    fun genericRuleWithCaretMatchesThroughIndex() {
        val blocker = blockerWith("/adframe.")
        assertTrue(blocker.blocks("https://cdn.example.com/some/adframe.x.js", "https://news.org/"))
        assertFalse(blocker.blocks("https://cdn.example.com/some/x.js", "https://news.org/"))
    }

    @Test
    fun mixedAnchoredAndGenericRulesBothBlock() {
        val blocker = blockerWith("||ads.example.com^", "-adframe-", "/-^")
        assertTrue(blocker.blocks("https://ads.example.com/a.js", "https://news.org/"))
        assertTrue(blocker.blocks("https://cdn.other.org/img-adframe-1.png", "https://news.org/"))
        assertTrue(blocker.blocks("https://cdn.other.org/x/-/y.js", "https://news.org/"))
    }

    @Test
    fun tokenlessRuleWhoseWholePatternIsAPlainRunStillMatches() {
        // "banner" has no interior run to key on, so it lands in the always-scanned
        // list and still matches inside the URL's longer run "banners".
        val blocker = blockerWith("banner")
        assertTrue(blocker.blocks("https://cdn.example.com/img/banners/top.png", "https://news.org/"))
    }
}
