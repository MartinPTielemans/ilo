package app.ilo.browser.android.blocking

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.assertFalse
import org.junit.Test

class FilterParserTest {

    private fun parseOne(line: String): NetworkRule? {
        val list = FilterParser.parse("t", line)
        return list.networkRules.singleOrNull()
    }

    @Test
    fun commentsAndHeaderAreSkipped() {
        val list = FilterParser.parse(
            "t",
            """
            [Adblock Plus 2.0]
            ! a comment
            ||ads.example.com^
            """.trimIndent()
        )
        assertEquals(1, list.networkRules.size)
    }

    @Test
    fun domainAnchorRule() {
        val rule = parseOne("||ads.example.com^")!!
        assertTrue(rule.anchorStart)
        assertEquals("ads.example.com^", rule.pattern)
        assertEquals(listOf("ads", "example", "com"), rule.domainTokens)
        assertFalse(rule.exception)
    }

    @Test
    fun pipeAnchorRule() {
        val rule = parseOne("|http://tracker.")!!
        assertTrue(rule.anchorStart)
        assertFalse(rule.anchorEnd)
    }

    @Test
    fun endAnchorRule() {
        val rule = parseOne("/banner.gif|")!!
        assertTrue(rule.anchorEnd)
        assertFalse(rule.anchorStart)
    }

    @Test
    fun plainSubstringRule() {
        val rule = parseOne("/adframe.")!!
        assertFalse(rule.anchorStart)
        assertFalse(rule.anchorEnd)
        assertEquals("/adframe.", rule.pattern)
    }

    @Test
    fun exceptionRule() {
        val rule = parseOne("@@||example.com/ads/\$image")!!
        assertTrue(rule.exception)
        assertEquals(setOf(ResourceType.IMAGE), rule.types)
    }

    @Test
    fun domainIncludeOption() {
        val rule = parseOne("||ads.com^\$domain=foo.com|bar.org")!!
        assertEquals(listOf("foo.com", "bar.org"), rule.includeDomains)
        assertTrue(rule.excludeDomains.isEmpty())
    }

    @Test
    fun domainExcludeOption() {
        val rule = parseOne("||ads.com^\$domain=~foo.com")!!
        assertEquals(listOf("foo.com"), rule.excludeDomains)
        assertTrue(rule.includeDomains.isEmpty())
    }

    @Test
    fun thirdPartyOption() {
        assertEquals(true, parseOne("||ads.com^\$third-party")!!.thirdParty)
        assertEquals(false, parseOne("||ads.com^$~third-party")!!.thirdParty)
        assertEquals(null, parseOne("||ads.com^")!!.thirdParty)
    }

    @Test
    fun resourceTypesMapped() {
        val rule = parseOne("||ads.com^\$script,image,stylesheet,subdocument")!!
        assertEquals(
            setOf(ResourceType.SCRIPT, ResourceType.IMAGE, ResourceType.STYLESHEET, ResourceType.SUB_FRAME),
            rule.types
        )
        assertEquals(null, parseOne("||ads.com^")!!.types)
    }

    @Test
    fun unsupportedOptionSkipsRule() {
        assertEquals(null, parseOne("||ads.com^\$popup"))
        assertEquals(null, parseOne("||ads.com^\$weirdoption"))
    }

    @Test
    fun trailingWildcardRuleBecomesPrefix() {
        val rule = parseOne("/ads/*")!!
        assertEquals("/ads/", rule.pattern)
    }

    @Test
    fun interiorWildcardSkipped() {
        assertEquals(null, parseOne("/ad*s/"))
    }

    @Test
    fun regexRuleSkipped() {
        assertEquals(null, parseOne("/^ad?s/"))
    }

    @Test
    fun cosmeticRulesParsedNotMatched() {
        val list = FilterParser.parse(
            "t",
            """
            example.com##.advert-banner
            example.com#@#.promoted
            example.com#?#div:style(color: red)
            example.com##^script:has-text(foo)
            """.trimIndent()
        )
        assertEquals(0, list.networkRules.size)
        assertEquals(4, list.cosmeticRules.size)
        assertTrue(list.cosmeticRules[1].exception)
        assertFalse(list.cosmeticRules[0].exception)
    }

    @Test
    fun emptyAndBlankLinesSkipped() {
        val list = FilterParser.parse("t", "\n  \n||a.com^\n")
        assertEquals(1, list.networkRules.size)
    }
}