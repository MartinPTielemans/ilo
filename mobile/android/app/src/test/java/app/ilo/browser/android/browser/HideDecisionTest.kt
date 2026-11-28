package app.ilo.browser.android.browser

import app.ilo.browser.android.blocking.HideStyles
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class HideDecisionTest {

    private val url = "https://news.example.com/a"

    @Test
    fun hidesWhenEverythingIsOn() {
        assertTrue(hidesWanted(url, blockingOn = true, hidesOn = true, siteOff = false, heldChoice = null, faultUp = false))
    }

    @Test
    fun noUrlNoMasterOrHideToggleMeansNoHides() {
        assertFalse(hidesWanted(null, true, true, false, null, false))
        assertFalse(hidesWanted(url, blockingOn = false, hidesOn = true, siteOff = false, heldChoice = null, faultUp = false))
        assertFalse(hidesWanted(url, true, hidesOn = false, siteOff = false, heldChoice = null, faultUp = false))
    }

    @Test
    fun siteExceptionStopsHidesUnlessTheTabHoldsOn() {
        assertFalse(hidesWanted(url, true, true, siteOff = true, heldChoice = null, faultUp = false))
        assertTrue(hidesWanted(url, true, true, siteOff = true, heldChoice = true, faultUp = false))
    }

    @Test
    fun heldOffStopsHidesEverywhere() {
        assertFalse(hidesWanted(url, true, true, siteOff = false, heldChoice = false, faultUp = false))
        assertFalse(hidesWanted(url, true, true, siteOff = true, heldChoice = false, faultUp = false))
    }

    @Test
    fun aWarningStandingInThePagesPlaceStopsHides() {
        assertFalse(hidesWanted(url, true, true, false, null, faultUp = true))
    }
}

class HideScriptTest {

    @Test
    fun emptyStylesGiveNoScript() {
        assertNull(HideScript.script(HideStyles.Styles("", "", 0, 0)))
    }

    @Test
    fun theScriptCarriesTheCssAndTheObserver() {
        val script = HideScript.script(HideStyles.Styles(".a{display:none !important}", ".b{display:none !important}", 1, 1))!!
        assertTrue(script.contains(".a{display:none !important}"))
        assertTrue(script.contains(".b{display:none !important}"))
        assertTrue(script.contains("MutationObserver"))
        assertTrue(script.contains("setTimeout"))
    }

    @Test
    fun cssWithQuotesRidesTheScriptEscaped() {
        val script = HideScript.script(HideStyles.Styles(".a[attr=\"x\"]{display:none !important}", "", 1, 1))!!
        assertTrue(script.contains("\\\"x\\\""))
    }
}