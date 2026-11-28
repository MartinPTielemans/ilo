package app.ilo.browser.android.browser

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import java.io.File
import kotlinx.coroutines.flow.MutableStateFlow

/**
 * The icons sites hand over, learned as their pages open, as the owner asked: a search engine's
 * mark shows the icon its own site gave, and a recent page's tile shows the page's. Kept in
 * memory and in a png named after the host, one host one icon, so every page of a site shares
 * it. Never asked of a private tab's page: its icon is kept of nothing.
 */
class FaviconStore(private val directory: File) {
    /** One thread for the disk, so writes to the same file queue instead of racing. */
    private val disk = java.util.concurrent.Executors.newSingleThreadExecutor()

    /** The icons in memory, keyed by host; a new one replaces an older of the same host. */
    private val icons = HashMap<String, Bitmap>()

    /** The hosts whose files were read, so each is read from disk once at most. */
    private val asked = HashSet<String>()

    /** Moves whenever an icon lands or goes, so marks and tiles recompose with it. */
    val version = MutableStateFlow(0)

    init { directory.mkdirs() }

    fun icon(of: String): Bitmap? = icons[host(of)]

    /** The hosts whose /favicon.ico was asked for, so each is asked once a launch. */
    private val sought = HashSet<String>()

    /** Reads a host's icon from disk, once, when its mark or tile first shows. */
    fun load(of: String) {
        val host = host(of)
        if (icons[host] != null || !asked.add(host)) return
        val file = file(host)
        if (!file.exists()) return
        val bitmap = runCatching { BitmapFactory.decodeFile(file.absolutePath) }.getOrNull() ?: return
        // One kept meanwhile is newer.
        if (icons[host] == null) {
            icons[host] = bitmap
            version.value++
        }
    }

    /** The /favicon.ico address a site's icon is asked at, kept as the address itself names it. */
    private fun faviconAddress(of: String): String? {
        val uri = runCatching { java.net.URI(of) }.getOrNull() ?: return null
        val scheme = uri.scheme ?: return null
        val site = uri.host ?: return null
        return "$scheme://$site/favicon.ico"
    }

    private val main = android.os.Handler(android.os.Looper.getMainLooper())

    /** Asks a site with no kept icon for its own /favicon.ico, as iOS's Favicons.find does:
     * the ask and the decode off the main thread, once a host a launch. */
    fun find(of: String) {
        val host = host(of)
        if (host.isEmpty() || icons[host] != null || !sought.add(host)) return
        val address = faviconAddress(of) ?: return
        disk.execute {
            val connection = runCatching {
                java.net.URI(address).toURL().openConnection() as java.net.HttpURLConnection
            }.getOrNull() ?: return@execute
            connection.connectTimeout = 5000
            connection.readTimeout = 5000
            val bytes = runCatching { connection.inputStream.use { it.readBytes() } }.getOrNull()
            connection.disconnect()
            if (bytes == null || bytes.isEmpty() || bytes.size > 2_000_000) return@execute
            val bitmap = runCatching { BitmapFactory.decodeByteArray(bytes, 0, bytes.size) }.getOrNull() ?: return@execute
            // The kept icon lands on the main thread, as a page's does from its WebView.
            main.post { if (icons[host] == null) keep(bitmap, of) }
        }
    }

    /** Keeps the icon a page gave for its host, so the engine that searches it shows it too. */
    fun keep(bitmap: Bitmap, of: String) {
        val host = host(of)
        if (host.isEmpty()) return
        if (icons[host] == bitmap) return
        icons.put(host, bitmap)
        version.value++
        // The write is off the main thread: the icon is already in memory for its mark.
        disk.execute { runCatching { file(host).writeBytes(toPng(bitmap)) } }
    }

    /** The host an address belongs to, its www dropped, the key its icon is kept under. */
    private fun host(url: String): String {
        val host = runCatching { java.net.URI(url).host }.getOrNull() ?: return ""
        return if (host.startsWith("www.")) host.substring(4) else host
    }

    private fun file(of: String) = File(directory, "$of.png")

    // PNG, as TabPictures does: an icon this small is worth a lossless file over a smaller one.
    private fun toPng(bitmap: Bitmap): ByteArray =
        java.io.ByteArrayOutputStream().let { out ->
            bitmap.compress(Bitmap.CompressFormat.PNG, 100, out)
            out.toByteArray()
        }
}