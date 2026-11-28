package app.ilo.browser.android.browser

import android.view.View

/** The host that shows a page's fullscreen video: the engine owns WebViews but has no window
 * of its own, so the activity adds the custom view over everything and takes it back. */
interface FullscreenHost {
    /** Puts `view` up over the whole window and hides the system bars. */
    fun showFullscreen(view: View)

    /** Takes the current fullscreen view down and brings the bars back; with none up, nothing. */
    fun hideFullscreen()
}

/** The custom view's state, kept apart from the window so WebView's rough edges land here once:
 * it can call the hide twice, and show a second view without hiding the first. */
class FullscreenState<V> {
    var current: Pair<V, () -> Unit>? = null
        private set

    val active: Boolean get() = current != null

    /** Shows `view`, remembering `onDown` for the real take-down; the caller takes the old
     * pair down before it shows, so a second show never strands a first callback. */
    fun show(view: V, onDown: () -> Unit) {
        current = view to onDown
    }

    /** Hides what is up; a second call with nothing up answers null, never running the exit twice. */
    fun hide(): Pair<V, () -> Unit>? {
        val old = current
        current = null
        return old
    }
}