package app.ilo.browser.android.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material3.Icon
import androidx.compose.material3.RadioButton
import androidx.compose.material3.RadioButtonDefaults
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.ilo.browser.android.R
import app.ilo.browser.android.browser.Browser
import app.ilo.browser.android.browser.SearchEngine

/**
 * The engine list as its own sheet: pick the default, add one of your own, edit or delete it,
 * where Settings' row leads and the field's chooser can open too.
 */
@Composable
fun EngineSheet(browser: Browser, onBack: () -> Unit) {
    val look = LocalLook.current
    // The editor carries the engine it changes, or none when it adds one.
    var editing by remember { mutableStateOf<SearchEngine?>(null) }
    var adding by remember { mutableStateOf(false) }
    var deleting by remember { mutableStateOf<SearchEngine?>(null) }

    if (adding || editing != null) {
        // Back inside the sheet returns to its list, the way a nested destination does.
        BackHandler { adding = false; editing = null }
        EngineEditor(browser, engine = editing, look) {
            adding = false; editing = null
        }
        return
    }
    if (deleting != null) {
        BackHandler { deleting = null }
        EngineDeleteAsk(deleting!!, look, onRefuse = { deleting = null }) {
            browser.settings.customEngines = browser.settings.customEngines - deleting!!
            deleting = null
        }
        return
    }
    BackHandler(onBack = onBack)

    Column(
        Modifier
            .fillMaxSize()
            .background(look.overlay)
            .windowInsetsPadding(WindowInsets.statusBars)
            .navigationBarsPadding()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = Metrics.Column.side)
    ) {
        Box(Modifier.heightIn(min = 48.dp), contentAlignment = Alignment.CenterStart) {
            PlateButton(Glyph.BACK, stringResource(R.string.back), look, onClick = onBack)
        }
        Text(
            stringResource(R.string.search_engine),
            color = look.text,
            fontSize = 20.sp,
            fontWeight = FontWeight.SemiBold,
            modifier = Modifier.padding(horizontal = Metrics.Column.inset, vertical = 8.dp),
        )
        // The blank custom list is fine: the shipped engines are still all there is to show.
        browser.settings.allEngines().forEach { engine ->
            EngineRow(
                engine = engine,
                browser = browser,
                look = look,
                onEdit = { editing = engine },
                onDelete = { deleting = engine },
            )
        }
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            modifier = Modifier
                .fillMaxWidth()
                .heightIn(min = 48.dp)
                .clickable(role = Role.Button) { adding = true }
                .padding(horizontal = Metrics.Column.inset),
        ) {
            Icon(Icons.Filled.Add, stringResource(R.string.engine_add), tint = look.accent)
            Text(stringResource(R.string.engine_add), color = look.text, fontSize = 16.sp, maxLines = 1)
        }
    }
}

/** One engine: its mark and name to choose, and its own edit and delete when it's the user's.
 * The row carries the picked state, so a screen reader gets one target that says it. */
@Composable
private fun EngineRow(
    engine: SearchEngine,
    browser: Browser,
    look: Look,
    onEdit: () -> Unit,
    onDelete: () -> Unit,
) {
    val chosen = engine.id == browser.settings.searchEngine.id
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(Metrics.Column.gap),
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 52.dp)
            .selectable(
                selected = chosen,
                role = Role.RadioButton,
                onClick = { browser.settings.searchEngine = engine },
            )
            .padding(horizontal = Metrics.Column.inset),
    ) {
        EngineMark(engine, browser.favicons, look)
        Text(
            engine.name,
            color = look.text,
            fontSize = 16.sp,
            maxLines = 1,
            modifier = Modifier.weight(1f),
        )
        if (browser.settings.customEngines.any { it.id == engine.id }) {
            // The shipped engines are fixed, so only the user's own carry these.
            Icon(
                Icons.Filled.Edit,
                stringResource(R.string.engine_edit),
                tint = look.soft,
                modifier = Modifier
                    .size(32.dp)
                    .clickable(role = Role.Button, onClick = onEdit)
                    .padding(6.dp),
            )
            Icon(
                Icons.Filled.Delete,
                stringResource(R.string.engine_delete),
                tint = look.soft,
                modifier = Modifier
                    .size(32.dp)
                    .clickable(role = Role.Button, onClick = onDelete)
                    .padding(6.dp),
            )
        }
        RadioButton(
            selected = chosen,
            onClick = null,
            colors = RadioButtonDefaults.colors(selectedColor = look.accent),
        )
    }
}

/** The add and edit form, one for both, its title the only thing that changes. */
@Composable
private fun EngineEditor(browser: Browser, engine: SearchEngine?, look: Look, onClose: () -> Unit) {
    var name by remember(engine) { mutableStateOf(engine?.name ?: "") }
    var address by remember(engine) { mutableStateOf(engine?.prefix ?: "") }
    val nameReady = name.isNotBlank()
    val addressReady = address.trim().startsWith("http")

    Column(
        Modifier
            .fillMaxSize()
            .background(look.overlay)
            .windowInsetsPadding(WindowInsets.statusBars)
            .navigationBarsPadding()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = Metrics.Column.side)
    ) {
        Box(Modifier.heightIn(min = 48.dp), contentAlignment = Alignment.CenterStart) {
            PlateButton(Glyph.BACK, stringResource(R.string.back), look, onClick = onClose)
        }
        Text(
            stringResource(if (engine == null) R.string.engine_add else R.string.engine_edit),
            color = look.text,
            fontSize = 20.sp,
            fontWeight = FontWeight.SemiBold,
            modifier = Modifier.padding(horizontal = Metrics.Column.inset, vertical = 8.dp),
        )
        // The label itself turns the alarm color when the field's words won't do, so no
        // invented sentence is needed to say what's missing.
        Text(
            stringResource(R.string.engine_name),
            color = if (nameReady) look.text else look.alarm,
            fontSize = 16.sp,
            modifier = Modifier.padding(horizontal = Metrics.Column.inset),
        )
        FieldPlate(look) {
            BasicField(
                value = name,
                onValueChange = { name = it },
                hint = "",
                look = look,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 12.dp),
            )
        }
        Text(
            stringResource(R.string.engine_address),
            color = if (addressReady) look.text else look.alarm,
            fontSize = 16.sp,
            modifier = Modifier.padding(horizontal = Metrics.Column.inset),
        )
        FieldPlate(look) {
            BasicField(
                value = address,
                onValueChange = { address = it },
                hint = "",
                look = look,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 12.dp),
                onSubmit = { saveEngine(browser, engine, name, address, onClose) },
            )
        }
        Spacer(Modifier.height(12.dp))
        Row(
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            modifier = Modifier.padding(horizontal = Metrics.Column.inset),
        ) {
            EngineChip(stringResource(R.string.close), look, onClick = onClose)
            EngineChip(
                stringResource(if (engine == null) R.string.engine_add else R.string.engine_edit),
                look,
            ) { saveEngine(browser, engine, name, address, onClose) }
        }
    }
}

/** The plate the editor's field sits on, as the other settings' fields are plated. */
@Composable
private fun FieldPlate(look: Look, content: @Composable () -> Unit) {
    Box(
        Modifier
            .fillMaxWidth()
            .heightIn(min = Metrics.Bar.height)
            .background(look.plate, RoundedCornerShape(Metrics.Bar.radius))
            .padding(vertical = 4.dp),
        contentAlignment = Alignment.CenterStart,
    ) { content() }
}

/** A word chip local to this sheet, as Settings' own chip with a description for none. */
@Composable
private fun EngineChip(label: String, look: Look, onClick: () -> Unit) {
    Box(
        modifier = Modifier
            .height(34.dp)
            .background(look.button, RoundedCornerShape(11.dp))
            .clickable(role = Role.Button, onClick = onClick)
            .padding(horizontal = 12.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(label, color = look.text, fontSize = 14.sp, fontWeight = FontWeight.SemiBold, maxLines = 1)
    }
}

/** Writes the engine when its words will do, an edit replacing the old one in its place. */
private fun saveEngine(
    browser: Browser,
    old: SearchEngine?,
    name: String,
    address: String,
    onDone: () -> Unit,
) {
    val trimmedName = name.trim()
    val trimmedAddress = address.trim()
    if (trimmedName.isEmpty() || !trimmedAddress.startsWith("http")) return
    // The id keeps a custom engine from standing in a shipped one's place, so a user's own
    // engine of the same name never shadows the built-in one the lists show first.
    val baseId = trimmedName.lowercase().filterNot { it == ' ' }
    val id = if (SearchEngine.all.any { it.id == baseId }) "$baseId-custom" else baseId
    val saved = SearchEngine(id, trimmedName, trimmedAddress)
    val kept = browser.settings.customEngines.filterNot { old != null && it.id == old.id }
    browser.settings.customEngines = kept + saved
    onDone()
}

/** The ask before a delete, so the one wrong tap can't take an engine away. */
@Composable
private fun EngineDeleteAsk(
    engine: SearchEngine,
    look: Look,
    onRefuse: () -> Unit,
    onConfirm: () -> Unit,
) {
    Column(
        Modifier
            .fillMaxSize()
            .background(look.overlay)
            .windowInsetsPadding(WindowInsets.statusBars)
            .navigationBarsPadding()
            .padding(horizontal = Metrics.Column.side + Metrics.Column.inset)
    ) {
        Text(
            stringResource(R.string.engine_delete),
            color = look.text,
            fontSize = 20.sp,
            fontWeight = FontWeight.SemiBold,
            modifier = Modifier.padding(top = 16.dp, bottom = 8.dp),
        )
        Text(engine.name, color = look.soft, fontSize = 16.sp, modifier = Modifier.padding(bottom = 16.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            EngineChip(stringResource(R.string.close), look, onClick = onRefuse)
            EngineChip(stringResource(R.string.engine_delete), look, onClick = onConfirm)
        }
    }
}