package app.ilo.browser.android.browser

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ExternalRouterTest {

    private val chrome =
        "intent://m.youtube.com/watch#Intent;scheme=vnd.youtube;package=com.google.android.youtube;" +
            "S.browser_fallback_url=https%3A%2F%2Fwww.youtube.com%2Fwatch;end"

    @Test
    fun `chrome form carries scheme package and fallback`() {
        val d = ExternalRouter.decide(chrome, true) as ExternalDecision.OpenInApp
        assertEquals("vnd.youtube://m.youtube.com/watch", d.resolvedIntentUri)
        assertEquals("youtube", d.appName)
        assertEquals("https://www.youtube.com/watch", d.fallbackUrl)
    }

    @Test
    fun `plain form targets the uri itself`() {
        val d = ExternalRouter.decide("intent://m.youtube.com/watch?id=x", true)
            as ExternalDecision.OpenInApp
        assertEquals("intent://m.youtube.com/watch?id=x", d.resolvedIntentUri)
        assertEquals("m.youtube.com", d.appName)
        assertNull(d.fallbackUrl)
    }

    @Test
    fun `plain form can still carry a fallback`() {
        val url = "intent://m.youtube.com/watch?S.browser_fallback_url=https%3A%2F%2Fyoutube.com%2Fw;end"
        val d = ExternalRouter.decide(url, true) as ExternalDecision.OpenInApp
        assertEquals("https://youtube.com/w", d.fallbackUrl)
    }

    @Test
    fun `malformed intent falls back to the page when present`() {
        val d = ExternalRouter.decide("intent://#Intent;S.browser_fallback_url=https%3A%2F%2Fexample.com;end", true)
        assertTrue(d is ExternalDecision.LoadInBrowser)
        assertEquals("https://example.com", (d as ExternalDecision.LoadInBrowser).url)
    }

    @Test
    fun `malformed intent without fallback stays`() {
        assertEquals(ExternalDecision.Stay, ExternalRouter.decide("intent://#Intent;end", true))
    }

    @Test
    fun `web pages are never the system's business`() {
        assertEquals(ExternalDecision.Stay, ExternalRouter.decide("https://ilo.dev/page", true))
        assertEquals(ExternalDecision.Stay, ExternalRouter.decide("http://ilo.dev/page", true))
    }

    @Test
    fun `setting off keeps today's behavior`() {
        assertEquals(ExternalDecision.Stay, ExternalRouter.decide(chrome, false))
        assertEquals(ExternalDecision.Stay, ExternalRouter.decide("tel:+34600000000", false))
    }

    @Test
    fun `other schemes go to the system`() {
        val d = ExternalRouter.decide("mailto:someone@example.com", true)
            as ExternalDecision.OpenInApp
        assertEquals("mailto:someone@example.com", d.resolvedIntentUri)
        assertEquals("someone@example.com", d.appName)
        val tel = ExternalRouter.decide("tel:+34600000000", true) as ExternalDecision.OpenInApp
        assertEquals("tel:+34600000000", tel.resolvedIntentUri)
    }

    @Test
    fun `no scheme at all stays`() {
        assertEquals(ExternalDecision.Stay, ExternalRouter.decide("about:blank", true))
        assertEquals(ExternalDecision.Stay, ExternalRouter.decide("  ", true))
    }

    @Test
    fun `scheme only in the facts without package uses the host`() {
        val d = ExternalRouter.decide("intent://m.youtube.com/watch#Intent;scheme=https;end", true)
            as ExternalDecision.OpenInApp
        assertEquals("https://m.youtube.com/watch", d.resolvedIntentUri)
        assertEquals("m.youtube.com", d.appName)
    }
}