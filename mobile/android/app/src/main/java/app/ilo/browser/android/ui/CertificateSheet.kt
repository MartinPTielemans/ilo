package app.ilo.browser.android.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.ilo.browser.android.R
import app.ilo.browser.android.browser.CertificateFault

/**
 * The certificate warning, ported from the iOS StatePage: its words and everything to press
 * sit at the foot of the screen, over the Space's ground, or over the ink a private page's
 * ground is. The sentence names what's wrong; the Details chip turns up the certificate's
 * facts, and only with them the way on, the Visit Anyway plate in the alarm color.
 */
@Composable
fun CertificateSheet(
    fault: CertificateFault,
    url: String?,
    look: Look,
    onAnswer: (allow: Boolean) -> Unit,
) {
    // The facts come up only when asked for, and a new certificate's are asked for again.
    var detailed by remember(fault) { mutableStateOf(false) }

    Column(modifier = Modifier.fillMaxWidth().padding(horizontal = Metrics.Column.side + Metrics.Column.inset)) {
        Text(
            text = stringResource(R.string.connection_not_private),
            color = look.text,
            fontSize = 24.sp,
            fontWeight = FontWeight.SemiBold,
            modifier = Modifier.padding(top = 16.dp, bottom = 8.dp),
        )
        Text(
            text = if (detailed) {
                fault.facts.sentence + " " + stringResource(R.string.go_on_only_if)
            } else {
                stringResource(R.string.certificate_reason, fault.exception.host)
            },
            color = look.soft,
            fontSize = 16.sp,
            modifier = Modifier.padding(bottom = 12.dp),
        )
        if (detailed) {
            Facts(fault, look)
            Spacer(Modifier.height(24.dp))
            Plate(
                text = stringResource(R.string.visit_anyway),
                look = look,
                warns = true,
                onClick = { onAnswer(true) },
            )
            Spacer(Modifier.height(8.dp))
            Plate(
                text = stringResource(R.string.go_back),
                look = look,
                onClick = { onAnswer(false) },
            )
        } else {
            url?.let {
                Text(
                    text = it,
                    color = look.soft,
                    fontSize = 13.sp,
                    fontFamily = FontFamily.Monospace,
                    maxLines = 2,
                    modifier = Modifier.padding(bottom = 12.dp),
                )
            }
            DetailsChip(look) { detailed = true }
            Spacer(Modifier.height(5.dp))
            Plate(
                text = stringResource(R.string.go_back),
                look = look,
                onClick = { onAnswer(false) },
            )
        }
        Spacer(Modifier.height(16.dp))
    }
}

/** The certificate's facts, each on its line under a rule, the labels soft as the iOS ones are. */
@Composable
private fun Facts(fault: CertificateFault, look: Look) {
    Column {
        Spacer(Modifier.height(12.dp))
        Box(
            Modifier
                .fillMaxWidth()
                .height(1.dp)
                .background(look.text.copy(alpha = 0.12f))
        )
        Spacer(Modifier.height(12.dp))
        for (line in fault.facts.lines(host = fault.exception.host)) {
            if (line.value != null) {
                Text(
                    text = buildAnnotatedString {
                        withStyle(SpanStyle(color = look.soft)) { append(line.label) }
                        append(" " + line.value)
                    },
                    color = look.text,
                    fontSize = 13.sp,
                    fontFamily = FontFamily.Monospace,
                    modifier = Modifier.padding(vertical = 2.dp),
                )
            } else {
                Text(
                    text = line.label,
                    color = look.text,
                    fontSize = 13.sp,
                    fontFamily = FontFamily.Monospace,
                    modifier = Modifier.padding(vertical = 2.dp),
                )
            }
        }
    }
}

/** The chip that turns up the certificate's facts, the way on only with them in view. */
@Composable
private fun DetailsChip(look: Look, onClick: () -> Unit) {
    Box(
        modifier = Modifier
            .heightIn(min = 48.dp)
            .clickable(role = Role.Button, onClick = onClick)
            .padding(end = Metrics.Column.inset),
        contentAlignment = Alignment.CenterStart,
    ) {
        Box(
            modifier = Modifier
                .height(34.dp)
                .background(look.button, RoundedCornerShape(11.dp))
                .semantics { contentDescription = "Details" },
            contentAlignment = Alignment.Center,
        ) {
            Text(
                text = "Details",
                color = look.text,
                fontSize = 14.sp,
                fontWeight = FontWeight.SemiBold,
                modifier = Modifier.padding(horizontal = 12.dp),
            )
        }
    }
}

/** A full-width button on a plate, as the iOS PlateButtonStyle: the warn one in the alarm color. */
@Composable
private fun Plate(
    text: String,
    look: Look,
    warns: Boolean = false,
    onClick: () -> Unit,
) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = Metrics.Bar.height)
            .background(look.plate, RoundedCornerShape(Metrics.Bar.radius))
            .clickable(role = Role.Button, onClick = onClick)
            .semantics { contentDescription = text },
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = text,
            color = if (warns) look.alarm else look.text,
            fontSize = 17.sp,
        )
    }
}