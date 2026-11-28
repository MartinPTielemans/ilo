package app.ilo.browser.android.blocking

import java.io.File
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * A hand-run measurement, like MatcherSpeedTest: it builds the hide state from the shipped
 * seed lists and prints the generic sheet's size and the host map's, for a person to read.
 */
class HideSheetSizeTest {

    @Test
    fun seedHideSheetMeasured() {
        val dir = File("src/main/assets/filters")
        val names = dir.listFiles { f -> f.name.endsWith(".txt") }?.map { it.name } ?: emptyList()
        assertTrue("no seed lists found", names.isNotEmpty())
        val lists = names.mapNotNull { name ->
            val kind = EasyListBlocker.Kind.entries.firstOrNull { name in it.files }?.label
            FilterParser.parse(name, File("src/main/assets/filters/$name").readText(), kind = kind)
        }
        val built = HideStyles.build(lists, EasyListBlocker.Kind.entries.map { it.label }.toSet())
        val hostBytes = built.byHost.values.sumOf { hides -> hides.sumOf { it.selector.length + 1 } }
        val exceptionSelectors = built.hostExceptions.values.sumOf { it.size }
        println(
            "hides: generic sheet ${built.genericCss.length} bytes / ${built.genericUsed} selectors kept, " +
                "${built.genericSkipped} cut, ${built.scriptletsSkipped} scriptlets skipped, " +
                "host map ${built.byHost.size} domains / $hostBytes bytes of selectors, " +
                "$exceptionSelectors per-host exception selectors"
        )
    }
}