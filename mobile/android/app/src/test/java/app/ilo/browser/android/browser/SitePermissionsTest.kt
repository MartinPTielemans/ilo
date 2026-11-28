package app.ilo.browser.android.browser

import app.ilo.browser.android.blocking.FakePrefs
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** The per-site permission answers: what a host holds, what the default decides, and how the
 * host's one spelling keeps www and bare addresses one site. */
class SitePermissionsTest {

    private fun store() = SitePermissionStore(FakePrefs())

    @Test
    fun anUnansweredSiteFallsToTheKindDefault() {
        val store = store()
        assertEquals(SiteAnswer.ASK, store.answer("example.com", SitePermission.CAMERA))
        store.setDefault(SitePermission.CAMERA, SiteAnswer.BLOCK)
        assertEquals(SiteAnswer.BLOCK, store.answer("example.com", SitePermission.CAMERA))
        // The site's own answer outranks the default it would otherwise take.
        store.setAnswer("example.com", SitePermission.CAMERA, SiteAnswer.ALLOW)
        assertEquals(SiteAnswer.ALLOW, store.answer("example.com", SitePermission.CAMERA))
    }

    @Test
    fun wwwAndBareAddressesAreOneSite() {
        val store = store()
        store.setAnswer("WWW.Example.com", SitePermission.LOCATION, SiteAnswer.ALLOW)
        assertEquals(SiteAnswer.ALLOW, store.answer("example.com", SitePermission.LOCATION))
        assertTrue(store.sitesAnswered().keys.single() == "example.com")
    }

    @Test
    fun aResetReturnsTheSiteToTheDefault() {
        val store = store()
        store.setAnswer("example.com", SitePermission.MICROPHONE, SiteAnswer.BLOCK)
        store.setAnswer("example.com", SitePermission.LOCATION, null)
        assertEquals(SiteAnswer.BLOCK, store.answer("example.com", SitePermission.MICROPHONE))
        store.clearSite("example.com")
        assertEquals(SiteAnswer.ASK, store.answer("example.com", SitePermission.MICROPHONE))
        assertTrue(store.sitesAnswered().isEmpty())
    }

    @Test
    fun theCycleWalksAskAllowBlock() {
        val store = store()
        var at = store.nextAnswer(SiteAnswer.ASK)
        assertEquals(SiteAnswer.ALLOW, at)
        at = store.nextAnswer(at)
        assertEquals(SiteAnswer.BLOCK, at)
        assertEquals(SiteAnswer.ASK, store.nextAnswer(at))
    }
}