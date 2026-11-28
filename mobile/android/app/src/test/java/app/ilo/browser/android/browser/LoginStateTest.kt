package app.ilo.browser.android.browser

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The save offer's decisions, as iOS's own tests do; the account's word stands in. */
class LoginStateTest {

    // The lane itself holds Android's SharedPreferences, so its never-list sits behind its
    // own door; these tests stand in for the save and fill state, where the decisions live.

    @Test
    fun originIgnoresDefaultPortsAndBracketlessIpv6() {
        assertEquals("https://example.com", LoginSave.origin("https://example.com/signin"))
        assertEquals("http://example.com:8080", LoginSave.origin("http://example.com:8080/x"))
        assertEquals("https://[::1]", LoginSave.origin("https://[::1]/"))
        assertNull(LoginSave.origin("about:blank"))
        assertNull(LoginSave.origin("file:///x"))
    }

    @Test
    fun formSentThenGoneMakesAChip() {
        val save = LoginSave(clock = { 1000L })
        save.committed("https://example.com/signin")
        save.used(UsedLogin("https://example.com", "me", "pw"), LoginSave.Judge { LoginKind.SAVE })
        assertNull(save.offer.value)
        // The page that follows shows no password field: the sign-in took.
        save.committed("https://example.com/home")
        save.heard(false, LoginSave.Judge { LoginKind.SAVE })
        val offer = save.offer.value
        assertTrue(offer != null && offer.phase == LoginSave.Phase.CHIP)
        assertEquals(LoginKind.SAVE, offer!!.kind)
    }

    @Test
    fun formBackOnNextPageIsNoSignIn() {
        val save = LoginSave(clock = { 1000L })
        save.committed("https://example.com/signin")
        save.used(UsedLogin("https://example.com", "me", "pw"), LoginSave.Judge { LoginKind.SAVE })
        save.committed("https://example.com/still-signin")
        save.heard(true, LoginSave.Judge { LoginKind.SAVE })
        assertNull(save.offer.value)
    }

    @Test
    fun editedFieldCancelsThePendingLogin() {
        val save = LoginSave(clock = { 1000L })
        save.committed("https://example.com/signin")
        save.used(UsedLogin("https://example.com", "me", "pw"), LoginSave.Judge { LoginKind.SAVE })
        save.edited("https://example.com")
        save.committed("https://example.com/next")
        save.heard(false, LoginSave.Judge { LoginKind.SAVE })
        assertNull(save.offer.value)
    }

    @Test
    fun judgeSaysNothingSavesNothing() {
        val save = LoginSave(clock = { 1000L })
        save.committed("https://example.com/signin")
        save.used(UsedLogin("https://example.com", "me", ""), LoginSave.Judge { null })
        save.committed("https://example.com/next")
        save.heard(false, LoginSave.Judge { null })
        assertNull(save.offer.value)
        assertFalse(save.holdsLogin())
    }

    @Test
    fun offerForAnotherSiteIsForgotten() {
        val save = LoginSave(clock = { 1000L })
        save.committed("https://example.com/signin")
        save.used(UsedLogin("https://example.com", "me", "pw"), LoginSave.Judge { LoginKind.SAVE })
        save.committed("https://example.com/next")
        save.heard(false, LoginSave.Judge { LoginKind.SAVE })
        assertTrue(save.offer.value != null)
        // The tab went elsewhere: the offer is for the site the tab was on.
        save.committed("https://other.com/")
        assertNull(save.offer.value)
    }

    @Test
    fun chipGoesToRowAndSavedGoesAway() {
        var clock = 1000L
        val save = LoginSave(clock = { clock })
        save.committed("https://example.com/signin")
        save.used(UsedLogin("https://example.com", "me", "pw"), LoginSave.Judge { LoginKind.SAVE })
        save.committed("https://example.com/next")
        save.heard(false, LoginSave.Judge { LoginKind.SAVE })
        assertEquals(LoginSave.Phase.CHIP, save.offer.value!!.phase)
        clock += 9000
        save.tick()
        assertEquals(LoginSave.Phase.ROW, save.offer.value!!.phase)
        // A hold keeps the chip past its time.
        save.setChip()
        clock += 9000
        save.tick()
        assertEquals(LoginSave.Phase.ROW, save.offer.value!!.phase)
    }

    @Test
    fun savingDropsThePasswordFromMemory() {
        var clock = 1000L
        val save = LoginSave(clock = { clock })
        save.committed("https://example.com/signin")
        save.used(UsedLogin("https://example.com", "me", "pw"), LoginSave.Judge { LoginKind.SAVE })
        save.committed("https://example.com/next")
        save.heard(false, LoginSave.Judge { LoginKind.SAVE })
        save.saved()
        assertEquals("", save.offer.value!!.login.password)
        assertEquals(LoginSave.Phase.SAVED, save.offer.value!!.phase)
        clock += 2000
        save.tick()
        assertNull(save.offer.value)
        assertFalse(save.holdsLogin())
    }

    @Test
    fun fillStateResetsPerCommitAndKeepsPutAway() {
        val fill = LoginState()
        fill.heard(true)
        assertTrue(fill.showsField.value)
        fill.putAwayOffer()
        assertTrue(fill.putAway.value)
        fill.committed()
        assertFalse(fill.showsField.value)
        assertFalse(fill.putAway.value)
    }

    @Test
    fun messageParsesTheFourShapes() {
        assertTrue((LoginMessage.parse("""{"shows":true}""") as LoginMessage.Shows).shows)
        val used = LoginMessage.parse("""{"login":{"username":"me","password":"pw"}}""") as LoginMessage.Used
        assertEquals("me", used.username)
        assertEquals("pw", used.password)
        assertEquals(LoginMessage.Left, LoginMessage.parse("""{"left":true}"""))
        assertEquals(LoginMessage.Edited, LoginMessage.parse("""{"edited":true}"""))
        assertNull(LoginMessage.parse("""{"other":true}"""))
        assertNull(LoginMessage.parse("not json"))
    }

    private fun LoginSave.setChip() {
        // The chip is back, held, so time doesn't move it.
        offer.value?.let { it.phase = LoginSave.Phase.CHIP; offer.value = it }
        hold()
    }
}
