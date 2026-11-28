package app.ilo.browser.android.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SignInWebScreenTest {

    private val redirect = "https://sync.example.com/signed-in"

    @Test
    fun matchesTheRedirectItself() {
        assertTrue(isSignInRedirect(redirect, redirect))
    }

    @Test
    fun matchesTheRedirectWithItsAnswer() {
        assertTrue(isSignInRedirect("https://sync.example.com/signed-in?code=x&state=y", redirect))
    }

    @Test
    fun refusesAnotherPathOnTheSameHost() {
        assertFalse(isSignInRedirect("https://sync.example.com/other", redirect))
    }

    @Test
    fun refusesAnotherHost() {
        assertFalse(isSignInRedirect("https://evil.example.com/signed-in", redirect))
    }

    @Test
    fun refusesAnotherScheme() {
        assertFalse(isSignInRedirect("http://sync.example.com/signed-in", redirect))
    }

    @Test
    fun matchesASchemeOrHostTypedInUpperCase() {
        // A keyboard that capitalizes is still the same address the server named.
        assertTrue(isSignInRedirect("HTTPS://SYNC.EXAMPLE.COM/signed-in?code=x", redirect))
    }

    @Test
    fun refusesAnotherPort() {
        assertFalse(isSignInRedirect("https://sync.example.com:8443/signed-in", redirect))
    }

    @Test
    fun refusesUnparseableAddresses() {
        assertFalse(isSignInRedirect("", redirect))
        assertFalse(isSignInRedirect("https://sync.example.com/signed-in", "not a uri"))
    }

    @Test
    fun hostLabelTakesTheHost() {
        assertEquals("sync.example.com", hostOf("https://sync.example.com/authorize?x=1"))
    }

    @Test
    fun hostLabelFallsBackToTheAddress() {
        assertEquals("not a uri", hostOf("not a uri"))
    }

    @Test
    fun payloadTakesTheQueryOfAFullUrl() {
        assertEquals(
            "code=X&state=Y",
            signInPayload("https://sync.example.com/signed-in?code=X&state=Y"))
    }

    @Test
    fun payloadPassesTheBareQueryThrough() {
        assertEquals("code=X&state=Y", signInPayload("code=X&state=Y"))
    }

    @Test
    fun payloadPassesAQuerylessUrlThrough() {
        assertEquals(
            "https://sync.example.com/signed-in",
            signInPayload("https://sync.example.com/signed-in"))
    }

    @Test
    fun payloadPassesAnUnparseableAddressThrough() {
        assertEquals("not a uri", signInPayload("not a uri"))
    }
}