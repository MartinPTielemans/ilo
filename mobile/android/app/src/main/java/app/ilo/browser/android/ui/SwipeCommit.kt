package app.ilo.browser.android.ui

/**
 * The travel a swipe release needs before it commits: a slow drag pays the full
 * base travel, a flick pays less, never less than a third of the base.
 */
fun swipeCommitTravel(deck: Float, releaseSpeed: Float, baseTravel: Float): Float {
    if (deck <= 0f) return baseTravel
    val s = kotlin.math.abs(releaseSpeed) / deck
    val relaxed = baseTravel * (1f - 0.35f * s)
    return maxOf(baseTravel / 3f, minOf(baseTravel, relaxed))
}