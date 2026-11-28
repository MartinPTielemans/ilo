package app.ilo.browser.android.browser

import app.ilo.browser.android.blocking.HideStyles

/**
 * The script that hides a page's ad elements: one style element with the generic and the
 * page's own selectors, then a debounced MutationObserver that re-adds it, because a page
 * that rewrites its own DOM can drop the element the observer never sees being dropped.
 * Selectors only, never code: a scriptlet is never put in a page.
 */
object HideScript {

    /** The script for `styles`, self-contained; null hides nothing. */
    fun script(styles: HideStyles.Styles): String? {
        if (styles.genericCss.isEmpty() && styles.hostCss.isEmpty()) return null
        val css = listOf(styles.genericCss, styles.hostCss)
            .filter { it.isNotEmpty() }.joinToString("")
        return script(css)
    }

    private fun script(css: String): String {
        val body = """
            (() => {
              const css = ${jsonString(css)};
              const apply = () => {
                let el = document.getElementById('ilo-hides');
                if (!el) {
                  el = document.createElement('style');
                  el.id = 'ilo-hides';
                  (document.head || document.documentElement).appendChild(el);
                }
                if (el.textContent !== css) el.textContent = css;
              };
              apply();
              if (globalThis.iloHideWatch) return;
              globalThis.iloHideWatch = true;
              let timer = 0;
              new MutationObserver(() => {
                if (timer) return;
                timer = setTimeout(() => { timer = 0; apply(); }, 100);
              }).observe(document.documentElement, { childList: true, subtree: true });
            })();
        """.trimIndent()
        return body
    }

    /** A JSON string literal, so the CSS rides the script without a quote of its own in it. */
    private fun jsonString(s: String): String {
        val sb = StringBuilder(s.length + 2).append('"')
        for (ch in s) when (ch) {
            '"' -> sb.append("\\\"")
            '\\' -> sb.append("\\\\")
            '\n' -> sb.append("\\n")
            '\r' -> sb.append("\\r")
            else -> sb.append(ch)
        }
        return sb.append('"').toString()
    }
}