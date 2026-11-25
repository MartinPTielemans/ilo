// A minimal JSON reader for the server's answers and the relay's frames: org.json isn't usable
// in the JVM unit tests, and the answers are flat objects of strings and numbers.

package app.ilo.browser.android.sync

object Json {
    /** An object, an array, a string, a Long or Double, a Boolean, or null. */
    fun parse(text: String): Any? = Parser(text).parseValue().also {
        it ?: return null
    }

    private class Parser(private val text: String) {
        private var i = 0

        fun parseValue(): Any? {
            skip()
            if (i >= text.length) return null
            return when (text[i]) {
                '{' -> parseObject()
                '[' -> parseArray()
                '"' -> parseString()
                else -> parseLiteral()
            }
        }

        private fun parseObject(): Map<String, Any?> {
            val out = LinkedHashMap<String, Any?>()
            i++  // '{'
            skip()
            if (i < text.length && text[i] == '}') {
                i++
                return out
            }
            while (i < text.length) {
                skip()
                val key = parseString() as? String ?: break
                skip()
                if (i >= text.length || text[i] != ':') break
                i++
                out[key] = parseValue()
                skip()
                if (i < text.length && text[i] == ',') {
                    i++
                    continue
                }
                if (i < text.length && text[i] == '}') i++
                break
            }
            return out
        }

        private fun parseArray(): List<Any?> {
            val out = ArrayList<Any?>()
            i++  // '['
            skip()
            if (i < text.length && text[i] == ']') {
                i++
                return out
            }
            while (i < text.length) {
                out.add(parseValue())
                skip()
                if (i < text.length && text[i] == ',') {
                    i++
                    continue
                }
                if (i < text.length && text[i] == ']') i++
                break
            }
            return out
        }

        private fun parseString(): String? {
            if (i >= text.length || text[i] != '"') return null
            i++
            val out = StringBuilder()
            while (i < text.length) {
                when (val c = text[i]) {
                    '"' -> {
                        i++
                        return out.toString()
                    }
                    '\\' -> {
                        i++
                        when (val escape = if (i < text.length) text[i] else return out.toString()) {
                            'n' -> out.append('\n')
                            't' -> out.append('\t')
                            'r' -> out.append('\r')
                            'b' -> out.append('\b')
                            'f' -> out.append('')
                            'u' -> {
                                if (i + 4 < text.length) {
                                    out.append(text.substring(i + 1, i + 5).toInt(16).toChar())
                                    i += 4
                                }
                            }
                            else -> out.append(escape)
                        }
                        i++
                    }
                    else -> {
                        out.append(c)
                        i++
                    }
                }
            }
            return out.toString()
        }

        private fun parseLiteral(): Any? {
            val start = i
            while (i < text.length && text[i] != ',' && text[i] != '}' && text[i] != ']') i++
            val literal = text.substring(start, i).trim()
            return when (literal) {
                "true" -> true
                "false" -> false
                "null" -> null
                else -> literal.toLongOrNull() ?: literal.toDoubleOrNull()
            }
        }

        private fun skip() {
            while (i < text.length && text[i].let { it == ' ' || it == '\t' || it == '\n' || it == '\r' }) i++
        }
    }
}

@Suppress("UNCHECKED_CAST")
fun jsonObject(text: String): Map<String, Any?>? =
    Json.parse(text) as? Map<String, Any?>

fun Map<String, Any?>.string(key: String): String? = (this[key] as? String)?.takeIf { it.isNotEmpty() }

fun Map<String, Any?>.long(key: String): Long? = when (val value = this[key]) {
    is Long -> value
    is Double -> value.toLong()
    else -> null
}

fun Any?.asObject(): Map<String, Any?>? = this as? Map<String, Any?>