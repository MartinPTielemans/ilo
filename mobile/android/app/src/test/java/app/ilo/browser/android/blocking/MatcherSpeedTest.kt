package app.ilo.browser.android.blocking

import java.io.File
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Not an assertion of the app's behavior: a timing harness, run by hand, that answers the
 * same requests many times against the shipped seed lists and prints the per-request cost,
 * so a matcher change says its own number. Not wired into CI: it asserts only that the
 * lists load and block, and its numbers are for a person to read.
 */
class MatcherSpeedTest {

    private fun seeds(): List<String> {
        val dir = File("src/main/assets/filters")
        return dir.listFiles { f -> f.name.endsWith(".txt") }?.map { it.name } ?: emptyList()
    }

    @Test
    fun seedListsLoadAndBlockMeasured() {
        val names = seeds()
        assertTrue("no seed lists found", names.isNotEmpty())
        val blocker = EasyListBlocker(FakePrefs())
        val lists = names.mapNotNull { name ->
            val text = File("src/main/assets/filters/$name").readText()
            val kind = EasyListBlocker.Kind.entries.firstOrNull { name in it.files }?.label
            FilterParser.parse(name, text, kind = kind)
        }
        blocker.replaceLists(lists)
        val page = "https://news.example.com/article"
        val requested = listOf(
            "https://news.example.com/styles/app.css",
            "https://cdn.news.example.com/js/app.js",
            "https://news.example.com/img/logo.png",
            "https://fonts.example.org/font.woff2",
            "https://api.maps.example.net/tiles/4/7/3.png",
            "https://news.example.com/api/headlines.json",
        )
        // Warm the engine and assert the lists actually block something: 007garage.com is a
        // rule the shipped EasyList seed carries.
        assertTrue(blocker.blocks("https://007garage.com/x.js", page))
        val warm = 20_000
        repeat(warm) { i -> blocker.blocks(requested[i % requested.size], page) }
        val rounds = 200_000
        val started = System.nanoTime()
        repeat(rounds) { i -> blocker.blocks(requested[i % requested.size], page) }
        val perCall = (System.nanoTime() - started) / rounds.toDouble() / 1_000.0
        println("matcher: $rounds requests in ${"%.1f".format(perCall)} us each (6 lists, ${lists.sumOf { it.networkRules.size }} network rules)")
    }
}
