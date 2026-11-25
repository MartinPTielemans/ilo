package app.ilo.browser.android.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

/** ilo's brand accent, the one color the app owns: the tokens' no-theme green. */
val Accent = Color(0xFF289054)

private val LightScheme = lightColorScheme(
    primary = Accent,
    onPrimary = Color.White,
    primaryContainer = Color(0xFFB8EBCB),
    onPrimaryContainer = Color(0xFF0B3B21),
    secondary = Color(0xFF5C6357),
    surface = Color(0xFFF3F0E8),
    onSurface = Color(0xFF16140F),
    surfaceVariant = Color(0xFFE4DFD2),
    onSurfaceVariant = Color(0xFF494537),
    background = Color(0xFFF3F0E8),
)

private val DarkScheme = darkColorScheme(
    primary = Color(0xFF57B97A),
    onPrimary = Color(0xFF0B3B21),
    primaryContainer = Color(0xFF1F4630),
    onPrimaryContainer = Color(0xFFB8EBCB),
    secondary = Color(0xFFBFC4B5),
    surface = Color(0xFF1F1D17),
    onSurface = Color(0xFFF3F0E8),
    surfaceVariant = Color(0xFF3A362C),
    onSurfaceVariant = Color(0xFFC9C4B4),
    background = Color(0xFF16140F),
)

/** ilo owns the look, so dynamic color stays off and the schemes are the app's own. */
@Composable
fun IloTheme(darkOverride: Boolean? = null, content: @Composable () -> Unit) {
    val dark = darkOverride ?: isSystemInDarkTheme()
    MaterialTheme(
        colorScheme = if (dark) DarkScheme else LightScheme,
        content = content,
    )
}