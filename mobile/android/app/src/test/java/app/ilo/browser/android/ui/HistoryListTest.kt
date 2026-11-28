package app.ilo.browser.android.ui

import app.ilo.browser.android.browser.HistoryEntry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.Calendar

class HistoryListTest {

    private fun dayStart(now: Long, daysAgo: Int): Long {
        val cal = Calendar.getInstance()
        cal.timeInMillis = now
        cal.set(Calendar.HOUR_OF_DAY, 0)
        cal.set(Calendar.MINUTE, 0)
        cal.set(Calendar.SECOND, 0)
        cal.set(Calendar.MILLISECOND, 0)
        return cal.timeInMillis - daysAgo * DAY_MS
    }

    @Test
    fun groupsByDaysBack() {
        val now = System.currentTimeMillis()
        val today = dayStart(now, 0)
        assertEquals(HistoryGroup.TODAY, historyGroupOf(today, now))
        assertEquals(HistoryGroup.TODAY, historyGroupOf(now, now))
        assertEquals(HistoryGroup.YESTERDAY, historyGroupOf(today - 1, now))
        assertEquals(HistoryGroup.WEEK, historyGroupOf(dayStart(now, 3), now))
        assertEquals(HistoryGroup.WEEK, historyGroupOf(dayStart(now, 7), now))
        assertEquals(HistoryGroup.MONTH, historyGroupOf(dayStart(now, 8), now))
        assertEquals(HistoryGroup.MONTH, historyGroupOf(dayStart(now, 30), now))
        assertEquals(HistoryGroup.OLDER, historyGroupOf(dayStart(now, 31), now))
    }

    @Test
    fun matchesByNameOrAddress() {
        val entry = HistoryEntry("https://example.org/page", "The Example Page")
        assertTrue(historyMatches(entry, ""))
        assertTrue(historyMatches(entry, "  "))
        assertTrue(historyMatches(entry, "example"))
        assertTrue(historyMatches(entry, "the example"))
        assertTrue(historyMatches(entry, "example.org/page"))
        assertFalse(historyMatches(entry, "nothing"))
    }
}