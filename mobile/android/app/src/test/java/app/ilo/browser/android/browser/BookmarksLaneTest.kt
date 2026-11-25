package app.ilo.browser.android.browser

import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** A stand-in account that records what the lane asked of it. Its `add` and `remove` answer
 * as the account would, without moving its own list: the lane hears of those only through
 * `takes` and `drops`, which bump the tick. */
private class FakeSite(
    kept: List<BookmarksLane.Kept> = emptyList(),
) : BookmarksLane.Site {
    val kepts = ArrayList(kept)
    val adds = ArrayList<Pair<String, String>>()
    val removes = ArrayList<String>()
    private val ticks = MutableStateFlow(0)
    override val tick: StateFlow<Int> = ticks.asStateFlow()
    /** The account has no word until its first download, as a signed-out one doesn't. */
    override var ready: Boolean = true

    override fun kept(): List<BookmarksLane.Kept> = kepts.map { it }
    override fun add(url: String, title: String): Boolean {
        adds.add(url to title)
        return true
    }
    override fun remove(url: String): Boolean {
        removes.add(url)
        return true
    }

    /** The account took a bookmark in from another device: the lane hears of it. */
    fun takes(url: String, title: String) {
        kepts.add(BookmarksLane.Kept(url, title, 3))
        ticks.value += 1
    }

    /** The account dropped a bookmark on another device: the lane hears of it. */
    fun drops(url: String) {
        kepts.removeIf { it.url == url }
        ticks.value += 1
    }
}

class BookmarksLaneTest {

    private fun stores(): PersonalStores {
        val dir = File.createTempFile("stores", null)
        dir.delete()
        dir.mkdirs()
        return PersonalStores(dir)
    }

    @Test
    fun theAccountListBecomesTheLocalOne() = runTest {
        val stores = stores()
        val scope = CoroutineScope(SupervisorJob() + UnconfinedTestDispatcher(testScheduler))
        val site = FakeSite(listOf(BookmarksLane.Kept("https://example.org/", "Example", 3)))
        BookmarksLane(stores, site, scope)
        assertTrue(stores.isBookmarked("https://example.org/"))
        scope.cancel()
    }

    @Test
    fun aLocalBookmarkReachesTheAccount() = runTest {
        val stores = stores()
        val scope = CoroutineScope(SupervisorJob() + UnconfinedTestDispatcher(testScheduler))
        val site = FakeSite()
        BookmarksLane(stores, site, scope)
        stores.bookmark("https://example.org/", "Example")
        assertTrue(site.adds.contains("https://example.org/" to "Example"))
        scope.cancel()
    }

    @Test
    fun aLocalUnbookmarkRemovesItThere() = runTest {
        val stores = stores()
        val scope = CoroutineScope(SupervisorJob() + UnconfinedTestDispatcher(testScheduler))
        val site = FakeSite(listOf(BookmarksLane.Kept("https://example.org/", "Example", 3)))
        BookmarksLane(stores, site, scope)
        assertTrue(stores.isBookmarked("https://example.org/"))
        stores.unbookmark("https://example.org/")
        assertTrue(site.removes.contains("https://example.org/"))
        scope.cancel()
    }

    @Test
    fun theSameListIsNeverPushedAgain() = runTest {
        val stores = stores()
        val scope = CoroutineScope(SupervisorJob() + UnconfinedTestDispatcher(testScheduler))
        val site = FakeSite(listOf(BookmarksLane.Kept("https://example.org/", "Example", 3)))
        BookmarksLane(stores, site, scope)
        assertTrue(stores.isBookmarked("https://example.org/"))
        assertEquals(0, site.adds.size)
        assertEquals(0, site.removes.size)
        scope.cancel()
    }

    @Test
    fun anAccountBookmarkOnAnotherDeviceArrives() = runTest {
        val stores = stores()
        val scope = CoroutineScope(SupervisorJob() + UnconfinedTestDispatcher(testScheduler))
        val site = FakeSite()
        BookmarksLane(stores, site, scope)
        site.takes("https://example.org/", "Example")
        assertTrue(stores.isBookmarked("https://example.org/"))
        assertEquals(0, site.adds.size)
        scope.cancel()
    }

    @Test
    fun anAccountBookmarkDroppedElsewhereGoesHere() = runTest {
        val stores = stores()
        val scope = CoroutineScope(SupervisorJob() + UnconfinedTestDispatcher(testScheduler))
        val site = FakeSite(listOf(BookmarksLane.Kept("https://example.org/", "Example", 3)))
        BookmarksLane(stores, site, scope)
        assertTrue(stores.isBookmarked("https://example.org/"))
        site.drops("https://example.org/")
        assertFalse(stores.isBookmarked("https://example.org/"))
        assertEquals(0, site.removes.size)
        scope.cancel()
    }

    @Test
    fun withoutAnAccountNothingIsTouched() = runTest {
        val stores = stores()
        val scope = CoroutineScope(SupervisorJob() + UnconfinedTestDispatcher(testScheduler))
        val lane = BookmarksLane(stores, null, scope)
        stores.bookmark("https://example.org/", "Example")
        assertTrue(stores.isBookmarked("https://example.org/"))
        lane.setAccount(null)
        assertTrue(stores.isBookmarked("https://example.org/"))
        scope.cancel()
    }

    @Test
    fun anAccountWithNoWordKeepsTheLocalList() = runTest {
        // A signed-out or still-downloading account answers empty: that must not read as
        // every bookmark deleted, which wiped the phone's own at cold start.
        val stores = stores()
        val scope = CoroutineScope(SupervisorJob() + UnconfinedTestDispatcher(testScheduler))
        val lane = BookmarksLane(stores, null, scope)
        stores.bookmark("https://example.org/", "Example")
        val site = FakeSite()
        site.ready = false
        lane.setAccount(site)
        assertTrue(stores.isBookmarked("https://example.org/"))
        site.ready = true
        site.takes("https://ilo.page/", "ilo")
        assertTrue(stores.isBookmarked("https://ilo.page/"))
        assertTrue(stores.isBookmarked("https://example.org/"))
        // The join also pushes the phone's own bookmarks to the account.
        assertTrue(site.adds.contains("https://example.org/" to "Example"))
        scope.cancel()
    }
}