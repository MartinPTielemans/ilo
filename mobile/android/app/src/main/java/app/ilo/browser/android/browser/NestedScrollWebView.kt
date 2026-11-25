package app.ilo.browser.android.browser

import android.content.Context
import android.view.MotionEvent
import android.view.VelocityTracker
import android.webkit.WebView
import androidx.core.view.NestedScrollingChild3
import androidx.core.view.NestedScrollingChildHelper
import androidx.core.view.ViewCompat

/**
 * I chose the nested pre-scroll variant that hands a top-pinned pull-down to the parent, so
 * ilo's own pull-to-refresh in Compose drives the gesture without fighting the page's scrolling.
 */
class NestedScrollWebView(context: Context) :
    WebView(context), NestedScrollingChild3 {

    private val scrollHelper = NestedScrollingChildHelper(this)

    init {
        isNestedScrollingEnabled = true
    }

    override fun setNestedScrollingEnabled(enabled: Boolean) {
        scrollHelper.isNestedScrollingEnabled = enabled
    }

    override fun isNestedScrollingEnabled(): Boolean = scrollHelper.isNestedScrollingEnabled

    override fun startNestedScroll(axes: Int, type: Int): Boolean =
        scrollHelper.startNestedScroll(axes, type)

    override fun stopNestedScroll(type: Int) {
        scrollHelper.stopNestedScroll(type)
    }

    override fun hasNestedScrollingParent(type: Int): Boolean =
        scrollHelper.hasNestedScrollingParent(type)

    // Newer androidx.core takes the consumed array as non-null and returns Unit, so the
    // result the helper gives has nowhere to go on this path.
    override fun dispatchNestedScroll(
        dxConsumed: Int,
        dyConsumed: Int,
        dxUnconsumed: Int,
        dyUnconsumed: Int,
        offsetInWindow: IntArray?,
        type: Int,
        consumed: IntArray
    ) {
        scrollHelper.dispatchNestedScroll(
            dxConsumed, dyConsumed, dxUnconsumed, dyUnconsumed, offsetInWindow, type, consumed
        )
    }

    override fun dispatchNestedScroll(
        dxConsumed: Int,
        dyConsumed: Int,
        dxUnconsumed: Int,
        dyUnconsumed: Int,
        offsetInWindow: IntArray?,
        type: Int
    ): Boolean = scrollHelper.dispatchNestedScroll(
        dxConsumed, dyConsumed, dxUnconsumed, dyUnconsumed, offsetInWindow, type
    )

    override fun dispatchNestedPreScroll(
        dx: Int,
        dy: Int,
        consumed: IntArray?,
        offsetInWindow: IntArray?,
        type: Int
    ): Boolean = scrollHelper.dispatchNestedPreScroll(dx, dy, consumed, offsetInWindow, type)

    override fun dispatchNestedFling(
        velocityX: Float,
        velocityY: Float,
        consumed: Boolean
    ): Boolean = scrollHelper.dispatchNestedFling(velocityX, velocityY, consumed)

    override fun dispatchNestedPreFling(velocityX: Float, velocityY: Float): Boolean =
        scrollHelper.dispatchNestedPreFling(velocityX, velocityY)

    // One pair per view is enough because the gesture is single-touch and short-lived.
    private val scrollOffsets = IntArray(2)
    private val scrollConsumed = IntArray(2)
    // The last MOVE's y, so the delta works for batched and unbatched streams alike: adb and
    // some screens deliver each move as its own event, with no historical batch to read.
    private var lastMoveY = 0f
    // The release's speed, so the parent hears the fling the pull ends in.
    private var velocityTracker: VelocityTracker? = null

    override fun onTouchEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                lastMoveY = event.y
                velocityTracker?.recycle()
                velocityTracker = VelocityTracker.obtain()
                velocityTracker?.addMovement(event)
                if (computeVerticalScrollOffset() <= 0) {
                    startNestedScroll(ViewCompat.SCROLL_AXIS_VERTICAL, ViewCompat.TYPE_TOUCH)
                }
            }
            MotionEvent.ACTION_MOVE -> {
                velocityTracker?.addMovement(event)
                // The delta is taken against raw coordinates: the consumed part is offset out of
                // the event below, and counting it into the delta would grow the pull by itself.
                val rawY = event.y
                // Only a page pinned at the top gives the pull to the parent, so page scrolling stays the page's own.
                // The parent takes the pull in View's own sign, where a downward finger is a negative scroll.
                if (computeVerticalScrollOffset() <= 0 && rawY > lastMoveY) {
                    val dy = (rawY - lastMoveY).toInt()
                    if (dispatchNestedPreScroll(
                            0,
                            -dy,
                            scrollConsumed,
                            scrollOffsets,
                            ViewCompat.TYPE_TOUCH
                        )
                    ) {
                        // The parent ate part of the pull, so the page only feels what is left.
                        event.offsetLocation(0f, scrollConsumed[1].toFloat())
                    }
                }
                lastMoveY = rawY
            }
            MotionEvent.ACTION_UP -> {
                // The release is the pull's verdict, so the parent hears the fling before the
                // gesture ends: the refresh fires on the finger leaving, as Chrome's does.
                velocityTracker?.addMovement(event)
                velocityTracker?.computeCurrentVelocity(1000)
                dispatchNestedPreFling(0f, velocityTracker?.yVelocity ?: 0f)
                velocityTracker?.recycle()
                velocityTracker = null
                stopNestedScroll(ViewCompat.TYPE_TOUCH)
            }
            MotionEvent.ACTION_CANCEL -> {
                velocityTracker?.recycle()
                velocityTracker = null
                stopNestedScroll(ViewCompat.TYPE_TOUCH)
            }
        }
        return super.onTouchEvent(event)
    }
}