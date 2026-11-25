package app.ilo.browser.android.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SwipeCommitTest {
    private val deck = 1000f
    private val base = 500f

    @Test fun zeroSpeedGivesBase() {
        assertEquals(base, swipeCommitTravel(deck, 0f, base), 0f)
    }

    @Test fun oneDeckPerSecondRelaxesTo65Percent() {
        assertEquals(base * 0.65f, swipeCommitTravel(deck, deck, base), 1e-4f)
    }

    @Test fun fastFlickHitsTheFloor() {
        assertEquals(base / 3f, swipeCommitTravel(deck, deck * 10f, base), 0f)
    }

    @Test fun neverAboveBase() {
        // The formula clamps at the base, so no speed, however signed, pays more than it.
        assertTrue(swipeCommitTravel(deck, -deck * 10f, base) <= base)
    }

    @Test fun negativeSpeedTreatedByAbs() {
        assertEquals(
            swipeCommitTravel(deck, deck, base),
            swipeCommitTravel(deck, -deck, base),
            0f,
        )
    }

    @Test fun tinyDeckNoNaN() {
        val travel = swipeCommitTravel(0.0001f, 5000f, base)
        assertFalse(travel.isNaN())
        assertFalse(travel < base / 3f - 1e-3f)
    }

    @Test fun zeroDeckFallsBackToBase() {
        assertEquals(base, swipeCommitTravel(0f, deck, base), 0f)
    }
}