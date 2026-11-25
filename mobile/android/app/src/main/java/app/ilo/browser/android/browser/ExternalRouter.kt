package app.ilo.browser.android.browser

import java.net.URLDecoder

/** The link's choice, decided before any Android Intent exists so it is unit-testable. */
sealed class ExternalDecision {
    /** The URI to hand the system, a name guess the dialog may better, and the fallback page. */
    data class OpenInApp(
        val resolvedIntentUri: String,
        val appName: String,
        val fallbackUrl: String?,
    ) : ExternalDecision()

    /** Load this page here instead of asking any app. */
    data class LoadInBrowser(val url: String) : ExternalDecision()

    data object Stay : ExternalDecision()
}

/** Decides by string parsing alone, so the caller builds the Android Intent only after the
 * user agrees. Facts sit after "#Intent;" as key=value pairs closed by ";end". */
object ExternalRouter {

    /** Off keeps today's behavior of staying; http(s) is never another app's business. */
    fun decide(url: String, openInApps: Boolean): ExternalDecision {
        val trimmed = url.trim()
        // mailto: and tel: carry no //, so the scheme is everything before the first colon.
        val scheme = trimmed.substringBefore(':').lowercase()
        if (scheme == "http" || scheme == "https") return ExternalDecision.Stay
        if (!openInApps) return ExternalDecision.Stay
        if (trimmed.startsWith("intent://")) return intent(trimmed)
        // mailto: and tel: carry no //: those known external schemes go to the system,
        // anything else without // (about:, data:) is the browser's own business.
        if (!trimmed.contains("://")) {
            if (scheme !in listOf("tel", "mailto", "sms", "geo", "market")) return ExternalDecision.Stay
            if (trimmed.substringAfter(':', "").isEmpty()) return ExternalDecision.Stay
        }
        // A scheme another app owns, or one no app answers: the caller resolves it and
        // falls back itself when nothing does.
        return ExternalDecision.OpenInApp(
            resolvedIntentUri = trimmed,
            appName = if (trimmed.contains("://")) {
                trimmed.substringAfter("://").substringBefore('/')
            } else {
                trimmed.substringAfter(':').substringBefore('/')
            },
            fallbackUrl = null,
        )
    }

    /** The two intent:// forms, with the facts the #Intent; section carries. */
    private fun intent(url: String): ExternalDecision {
        val sectionAt = url.indexOf("#Intent;")
        val fallback = fallbackOf(url)
        if (sectionAt < 0) {
            // Plain form: the URI itself is the target, its host naming the app.
            return ExternalDecision.OpenInApp(
                resolvedIntentUri = url,
                appName = url.substringAfter("intent://").substringBefore('/'),
                fallbackUrl = fallback,
            )
        }
        val facts = factsOf(url.substring(sectionAt + "#Intent;".length))
        val scheme = facts["scheme"]
        val pkg = facts["package"]
        // Without a scheme to land on or a package to name, the link points nowhere.
        if (scheme == null && pkg == null) return fallbackOr(fallback)
        val before = url.substring(0, sectionAt)
        val resolved = if (scheme != null) before.replaceFirst("intent:", "$scheme:") else before
        return ExternalDecision.OpenInApp(
            resolvedIntentUri = resolved,
            appName = pkg?.substringAfterLast('.') ?: hostOf(before),
            fallbackUrl = fallback,
        )
    }

    /** The fallback page, URL-decoded from the form intent: links pack values in. */
    private fun fallbackOf(url: String): String? {
        val named = url.substringAfter("browser_fallback_url=", "").substringBefore(';')
        if (named.isEmpty() || named == url) return null
        return runCatching { URLDecoder.decode(named, "UTF-8") }.getOrDefault(named)
    }

    /** The #Intent; section's key=value pairs, ended by ";end" or the string's end. */
    private fun factsOf(section: String): Map<String, String> {
        val body = section.substringBefore(";end")
        val facts = mutableMapOf<String, String>()
        for (part in body.split(';')) {
            val eq = part.indexOf('=')
            if (eq > 0) facts[part.substring(0, eq)] = part.substring(eq + 1)
        }
        return facts
    }

    private fun fallbackOr(fallback: String?): ExternalDecision =
        if (fallback != null) ExternalDecision.LoadInBrowser(fallback) else ExternalDecision.Stay

    private fun hostOf(before: String): String =
        before.substringAfter("//").substringBefore('/').substringBefore('?')
}