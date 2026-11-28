package app.ilo.browser.android.blocking

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class HideStylesTest {

    private fun listsOf(vararg lines: String, kind: String? = null) =
        listOf(FilterParser.parse("t", lines.joinToString("\n"), kind = kind))

    @Test
    fun genericSelectorsLandInTheSheet() {
        val built = HideStyles.build(listsOf("##.advert-banner", "##.ad-box"), emptySet())
        assertTrue(built.genericCss.contains(".advert-banner"))
        assertTrue(built.genericCss.contains(".ad-box"))
        assertTrue(built.genericCss.endsWith("{display:none !important}"))
        assertEquals(2, built.genericUsed)
        assertEquals(0, built.genericSkipped)
    }

    @Test
    fun domainlessExceptionRemovesTheSelectorEverywhere() {
        val built = HideStyles.build(listsOf("##.advert-banner", "#@#.advert-banner"), emptySet())
        assertFalse(built.genericCss.contains(".advert-banner"))
        assertEquals(1, built.genericSkipped)
    }

    @Test
    fun perHostExceptionRestrainsTheGenericSheetForThatHostOnly() {
        val built = HideStyles.build(listsOf("##.advert-banner", "example.com#@#.advert-banner"), emptySet())
        // The generic selector stays in the sheet; example.com alone is spared it.
        assertTrue(built.genericCss.contains(".advert-banner"))
        val onExample = HideStyles.forHost(built, "sub.example.com")
        assertFalse(onExample.genericCss.contains(".advert-banner"))
        val elsewhere = HideStyles.forHost(built, "other.org")
        assertTrue(elsewhere.genericCss.contains(".advert-banner"))
        // The same exception takes the selector from that host's own map too.
        val withOwn = HideStyles.build(listsOf("example.com##.advert-banner", "example.com#@#.advert-banner"), emptySet())
        assertTrue(HideStyles.hostSelectors(withOwn, "a.example.com").isEmpty())
        assertTrue(HideStyles.hostSelectors(withOwn, "b.org").isEmpty())
    }

    @Test
    fun scriptletsAreCountedNeverRun() {
        val built = HideStyles.build(listsOf("example.com##+js(set-constant, ads)", "example.com##.banner"), emptySet())
        assertEquals(1, built.scriptletsSkipped)
        assertTrue(HideStyles.hostSelectors(built, "example.com").contains(".banner"))
        assertFalse(HideStyles.hostSelectors(built, "example.com").contains("+js(set-constant, ads)"))
    }

    @Test
    fun kindGatingLeavesAKindsHidesOut() {
        val built = HideStyles.build(
            listOf(FilterParser.parse("t", "##.advert-banner", kind = "ads")),
            setOf("trackers"),
        )
        assertEquals(0, built.genericUsed)
        assertTrue(built.genericCss.isEmpty())
        val on = HideStyles.build(
            listOf(FilterParser.parse("t", "##.advert-banner", kind = "ads")),
            setOf("ads"),
        )
        assertTrue(on.genericCss.contains(".advert-banner"))
    }

    @Test
    fun sizeBudgetCutsLeastFrequentFirst() {
        // Rare rules long enough to pass the budget as a group, and one common survivor.
        val lines = (1..5000).map { "##.rare-selector-with-a-long-name-number-$it" }
        val all = lines + List(50) { "##.common-selector" }
        val built = HideStyles.build(listsOf(*all.toTypedArray()), emptySet())
        assertTrue(built.genericCss.length <= HideStyles.GENERIC_BUDGET)
        assertTrue(built.genericCss.contains(".common-selector"))
        assertTrue(built.genericSkipped > 0)
        assertEquals(1, built.genericCss.split(",").count { it.contains("common-selector") })
        // The one most-occurring selector is kept first, so it survives the cut.
        assertTrue(built.genericUsed < 5001)
    }

    @Test
    fun hostMapFindsSubdomainsAndHonorsExceptions() {
        val built = HideStyles.build(
            listsOf(
                "example.com##.site-ad",
                "example.com##.side-ad",
                "sub.example.com##.inner-ad",
                "example.com#@#.site-ad",
            ),
            emptySet(),
        )
        // The exception cancels its selector on that host; the host keeps its other one.
        assertTrue(HideStyles.hostSelectors(built, "a.example.com").none { it == ".site-ad" })
        assertTrue(HideStyles.hostSelectors(built, "a.example.com").contains(".side-ad"))
        assertTrue(HideStyles.hostSelectors(built, "deep.sub.example.com").contains(".inner-ad"))
        val styles = HideStyles.forHost(built, "deep.sub.example.com")
        assertTrue(styles.hostCss.contains(".inner-ad"))
        assertFalse(styles.hostCss.contains(".site-ad"))
    }

    @Test
    fun excludedSubdomainDoesNotGetTheRulesHides() {
        val built = HideStyles.build(listsOf("example.com##.site-ad", "example.com,~pay.example.com##.pay-shield"), emptySet())
        val onSub = HideStyles.hostSelectors(built, "a.example.com")
        assertTrue(onSub.contains(".site-ad"))
        assertTrue(onSub.contains(".pay-shield"))
        // The excluded subdomain misses its parent's rule, and a stranger host gets nothing.
        val onPay = HideStyles.hostSelectors(built, "pay.example.com")
        assertFalse(onPay.contains(".pay-shield"))
        assertTrue(onPay.contains(".site-ad"))
        assertFalse(HideStyles.hostSelectors(built, "notexample.com").contains(".site-ad"))
    }

    @Test
    fun aSelectorThatWouldBreakOutIsDroppedWhole() {
        val built = HideStyles.build(listsOf("##.fine", "##.broken{position:fixed}"), emptySet())
        assertTrue(built.genericCss.contains(".fine"))
        assertFalse(built.genericCss.contains("broken"))
        assertEquals(1, built.scriptletsSkipped)
    }

    @Test
    fun emptyBuildHidesNothing() {
        val built = HideStyles.build(emptyList(), emptySet())
        assertTrue(built.genericCss.isEmpty())
        assertTrue(built.byHost.isEmpty())
        val styles = HideStyles.forHost(built, "x.com")
        assertTrue(styles.genericCss.isEmpty() && styles.hostCss.isEmpty())
    }
}