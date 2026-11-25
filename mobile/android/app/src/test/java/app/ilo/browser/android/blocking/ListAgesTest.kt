package app.ilo.browser.android.blocking

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** A built-in list keeps when it last fetched well, and a week without a fetch makes it stale. */
class ListAgesTest {

    private fun blocker() = EasyListBlocker(FakePrefs())

    @Test
    fun aListThatNeverFetchedIsStaleAndHasNoTime() {
        val blocker = blocker()
        assertEquals(0L, blocker.listAt("easylist.txt"))
        assertTrue(blocker.listStale("easylist.txt"))
    }

    @Test
    fun aListFetchedTodayIsFresh() {
        val blocker = blocker()
        val prefs = blocker.javaClass.getDeclaredField("prefs")
        prefs.isAccessible = true
        val store = prefs.get(blocker) as android.content.SharedPreferences
        store.edit().putLong("list_updated_easylist.txt", System.currentTimeMillis()).apply()
        assertFalse(blocker.listStale("easylist.txt"))
        assertTrue(blocker.listStale("easylist.txt", System.currentTimeMillis() + 8L * 24 * 3600 * 1000))
    }

    @Test
    fun theTimesFlowHoldsEveryBuiltInList() {
        val blocker = blocker()
        val names = EasyListBlocker.Kind.entries.flatMap { it.files }
        assertEquals(names.associateWith { 0L }, blocker.listTimesFlow.value)
    }
}
