package app.ilo.browser.android.browser

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class TabsTest {
    private fun state() = BrowserState(createTempDir())

    @Test
    fun aNewTabIsSelected() {
        val state = state()
        val tab = state.newTab("https://example.com")
        assertEquals(tab.id, state.selectedId.value)
        assertTrue(state.tabs.value.contains(tab))
    }

    @Test
    fun aPlusTabSurvivesARelaunch() {
        // The + makes its tab before the field says where it goes, so a relaunch between the
        // press and the address keeps the tab, its start page where it stood.
        val dir = createTempDir()
        val made = BrowserState(dir).newTab("https://ilo.page")
        val relaunched = BrowserState(dir)
        assertTrue(relaunched.tabs.value.any { it.id == made.id })
        assertEquals(made.id, relaunched.selectedId.value)
    }

    @Test
    fun closingTheSelectedTabBringsUpAnother() {
        val state = state()
        val first = state.newTab("https://example.com")
        val second = state.newTab("https://example.org")
        state.close(second.id)
        assertEquals(first.id, state.selectedId.value)
    }

    @Test
    fun closingTheLastTabSelectsNone() {
        val state = state()
        val tab = state.newTab("https://example.com")
        state.close(tab.id)
        assertNull(state.selectedId.value)
    }

    @Test
    fun tabsBelongToSpaces() {
        val state = state()
        state.newTab("https://example.com", space = state.currentSpace.value)
        val other = state.addSpace("Work")
        val work = state.newTab("https://work.example", space = other.id)
        state.selectSpace(state.spaces.value.first().id)
        assertFalse(state.tabsIn().any { it.id == work.id })
        state.selectSpace(other.id)
        assertTrue(state.tabsIn().any { it.id == work.id })
    }

    @Test
    fun aTabSwitchChangesOnlyTheSelection() {
        val state = state()
        val first = state.newTab("https://example.com")
        val second = state.newTab("https://example.org")
        state.select(first.id)
        assertEquals(first.id, state.selectedId.value)
        assertEquals(listOf(first, second), state.tabs.value)
    }

    @Test
    fun theSessionSurvivesANewState() {
        val dir = createTempDir()
        val first = BrowserState(dir)
        val tab = first.newTab("https://example.com")
        first.update(tab.copy(title = "Example"))

        val second = BrowserState(dir)
        assertEquals(1, second.tabs.value.size)
        assertEquals("https://example.com", second.tabs.value.first().url)
        assertEquals("Example", second.tabs.value.first().title)
        assertEquals(second.tabs.value.first().id, second.selectedId.value)
    }

    @Test
    fun aTabAskingForTheDesktopComesBackAsking() {
        val dir = createTempDir()
        val first = BrowserState(dir)
        val tab = first.newTab("https://example.com")
        first.update(tab.copy(wantsDesktop = true))

        val second = BrowserState(dir)
        assertTrue(second.tabs.value.first().wantsDesktop)
    }

    @Test
    fun aPrivateTabIsNeverPersisted() {
        val dir = createTempDir()
        val first = BrowserState(dir)
        first.newTab("https://example.com", isPrivate = true)
        val second = BrowserState(dir)
        assertTrue(second.tabs.value.isEmpty())
        assertTrue(second.tabs.value.none { it.isPrivate })
    }

    @Test
    fun aFailedSessionWriteSurfacesItsError() {
        val dir = createTempDir()
        val file = java.io.File(dir, "session.json")
        file.parentFile.mkdirs()
        file.writeText("in the way")
        // A directory where the session file belongs makes the write fail.
        file.delete()
        file.mkdirs()
        val state = BrowserState(dir)
        state.newTab("https://example.com")
        assertTrue(state.persistError.value != null)
    }

    @Test
    fun aGoodSessionWriteClearsTheError() {
        val state = state()
        state.newTab("https://example.com")
        assertNull(state.persistError.value)
    }

    @Test
    fun aPrivateTabHasItsOwnExceptions() {
        val state = state()
        val priv = state.newTab(isPrivate = true)
        val normal = state.newTab()
        assertTrue(state.exceptionsFor(priv) !== state.exceptionsFor(normal))
        assertEquals(state.exceptions, state.exceptionsFor(normal))
    }

    @Test
    fun aClosedTabDropsItsTrustedException() {
        val state = state()
        val tab = state.newTab("https://example.com")
        val exception = CertificateException("example.com", 443, ByteArray(32))
        state.tabExceptions.value = mapOf(tab.id to exception)
        state.close(tab.id)
        assertTrue(state.tabExceptions.value.isEmpty())
    }

    @Test
    fun aTabWarningIsItsOwnAndDiesWithIt() {
        val state = state()
        val first = state.newTab("https://example.com")
        val second = state.newTab("https://other.example")
        val fault = CertificateFault(
            CertificateException("example.com", 443, ByteArray(32)),
            CertificateFacts("CN=example.com", null, true, listOf(CertificateFacts.Fault.UNKNOWN_SIGNER)),
        )
        state.tabFaults.value = mapOf(first.id to fault)
        assertNull(state.tabFaults.value[second.id])
        state.tabFaults.value = state.tabFaults.value + (second.id to fault)
        assertEquals(fault, state.tabFaults.value[first.id])
        state.close(first.id)
        assertNull(state.tabFaults.value[first.id])
        assertEquals(fault, state.tabFaults.value[second.id])
    }

    @Test
    fun beyondTheLiveLimitTheOldestTabFreezes() {
        val state = state()
        state.liveLimit = 2
        var frozen: Tab? = null
        state.freeze = { frozen = it }
        val a = state.newTab()
        val b = state.newTab()
        val c = state.newTab()
        assertEquals(a.id, frozen?.id)
        assertEquals(listOf(a.id, b.id, c.id).sorted(), state.tabs.value.map { it.id }.sorted())
    }

    @Test
    fun privateModeMakesNewTabsPrivate() {
        val state = state()
        state.privateMode.value = true
        val tab = state.newTab("https://example.com")
        assertTrue(tab.isPrivate)
    }

    @Test
    fun privateModeEndsWithItsLastTab() {
        val state = state()
        state.privateMode.value = true
        val priv = state.newTab(isPrivate = true)
        val other = state.newTab(isPrivate = true)
        state.close(priv.id)
        assertTrue(state.privateMode.value)
        state.close(other.id)
        assertFalse(state.privateMode.value)
    }

    @Test
    fun closingAPrivateTabBringsUpAnotherPrivateOne() {
        val state = state()
        val normal = state.newTab("https://example.com")
        val priv = state.newTab(isPrivate = true)
        val otherPriv = state.newTab(isPrivate = true)
        state.close(otherPriv.id)
        assertEquals(priv.id, state.selectedId.value)
        // The last private tab gone, the Space shows itself rather than the other kind's page.
        state.close(priv.id)
        assertNull(state.selectedId.value)
        assertEquals(normal.id, state.tabs.value.first().id)
    }

    @Test
    fun closingTheLastPrivateTabShowsTheSpace() {
        val state = state()
        state.newTab("https://example.com")
        val priv = state.newTab(isPrivate = true)
        state.close(priv.id)
        assertNull(state.selectedId.value)
        // The mode leaves with its last tab, and the ordinary tab is the grid's again.
        assertFalse(state.privateMode.value)
    }

    @Test
    fun showsPrivateFollowsTheScreen() {
        val state = state()
        assertFalse(state.showsPrivate())
        val priv = state.newTab(isPrivate = true)
        assertTrue(state.showsPrivate())
        state.close(priv.id)
        assertFalse(state.showsPrivate())
    }

    @Test
    fun aPrivateModeSessionSelectsItsOwnKind() {
        val state = state()
        val normal = state.newTab("https://example.com")
        state.privateMode.value = true
        val priv = state.newTab(isPrivate = true)
        state.select(normal.id)
        state.selectSpace(state.spaces.value.first().id)
        assertEquals(priv.id, state.selectedId.value)
    }
}

private fun createTempDir(): File = kotlin.io.path.createTempDirectory("ilo-test").toFile()