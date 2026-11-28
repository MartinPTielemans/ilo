package app.ilo.browser.android.browser

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import java.io.File
import kotlinx.coroutines.flow.MutableStateFlow

/**
 * What the phone remembers of a tab without a WebView: a picture of its page for its card in
 * the Space and the deck, kept in memory and in a png file named after the tab's id, gone
 * when the tab is, as the iOS TabCache does. A private tab is never asked for one.
 */
class TabPictures(private val directory: File) {
    /** One thread for the disk, so writes to the same file queue instead of racing. */
    private val disk = java.util.concurrent.Executors.newSingleThreadExecutor()

    /** The pictures in memory, keyed by tab id; a new one replaces an older. */
    private val pictures = HashMap<String, Bitmap>()

    /** The ids whose files were read, so each is read from disk once at most. */
    private val asked = HashSet<String>()

    /** Moves whenever a picture lands or goes, so cards recompose with it. */
    val version = MutableStateFlow(0)

    init { directory.mkdirs() }

    fun picture(of: String): Bitmap? = pictures[of]

    /** Reads a tab's picture from disk, once, when its card first shows. */
    fun load(of: String) {
        if (pictures[of] != null || !asked.add(of)) return
        val file = file(of)
        if (!file.exists()) return
        val bitmap = runCatching { BitmapFactory.decodeFile(file.absolutePath) }.getOrNull() ?: return
        // One taken meanwhile is newer.
        if (pictures[of] == null) {
            pictures[of] = bitmap
            version.value++
        }
    }

    fun keep(bitmap: Bitmap, of: String) {
        pictures.put(of, bitmap)
        version.value++
        // The write is off the main thread: the picture is already in memory for its card.
        disk.execute { runCatching { file(of).writeBytes(toPng(bitmap)) } }
        // The replaced bitmap is left to GC: a frame already drawing it would crash on a recycle.
    }

    /** Lets go of a tab's picture alone, when its page gave way to something the picture isn't of. */
    fun forget(of: String) {
        if (pictures.remove(of) == null && !file(of).exists()) return
        asked.add(of)
        version.value++
        disk.execute { runCatching { file(of).delete() } }
        // The removed bitmap is left to GC, as in keep: a recycle can race a drawn frame.
    }

    /** Drops the files of tabs no longer open, as the iOS cache prunes to the account's tabs,
     * and the jpg files of an older build with them: a picture's name is a png now. */
    fun prune(keeping: Collection<String>) {
        val keepSet = keeping.toSet()
        for (id in pictures.keys.toList()) {
            if (id !in keepSet) { pictures.remove(id) }
        }
        directory.listFiles()?.forEach { file ->
            val id = file.nameWithoutExtension
            if (id !in keepSet || !file.name.endsWith(".png")) { runCatching { file.delete() } }
        }
        version.value++
        // Dropped bitmaps are left to GC, as in keep: a recycle can race a drawn frame.
    }

    private fun file(of: String) = File(directory, "$of.png")

    // PNG rather than a hard-compressed jpeg: a card-sized picture the deck shows full width
    // lost its lettering to the 60% jpeg, and a cache file that stays crisp is worth its size.
    private fun toPng(bitmap: Bitmap): ByteArray =
        java.io.ByteArrayOutputStream().let { out ->
            bitmap.compress(Bitmap.CompressFormat.PNG, 100, out)
            out.toByteArray()
        }
}