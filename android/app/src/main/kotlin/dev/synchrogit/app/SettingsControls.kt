package dev.synchrogit.app

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp

@Composable fun Field(label: String, value: String, enabled: Boolean, secret: Boolean = false, multiline: Boolean = false, onChange: (String) -> Unit) {
    OutlinedTextField(value, onChange, modifier = Modifier.fillMaxWidth(), label = { Text(label) }, enabled = enabled,
        singleLine = !multiline, visualTransformation = if (secret) PasswordVisualTransformation() else VisualTransformation.None)
}

@Composable fun Toggle(label: String, value: Boolean, enabled: Boolean, onChange: (Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth().heightIn(min = 56.dp)
        .toggleable(value = value, enabled = enabled, role = Role.Switch, onValueChange = onChange)
        .padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        Text(label, Modifier.weight(1f), color = if (enabled) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.onSurface.copy(alpha = 0.38f))
        Switch(checked = value, onCheckedChange = null, enabled = enabled)
    }
}

@Composable fun ChoiceField(label: String, selected: String, options: List<Pair<String, String>>, enabled: Boolean, onChange: (String) -> Unit) {
    var expanded by remember { mutableStateOf(false) }
    Box(Modifier.fillMaxWidth()) {
        OutlinedButton(onClick = { expanded = true }, enabled = enabled,
            shape = MaterialTheme.shapes.small,
            modifier = Modifier.fillMaxWidth().testTag(label), contentPadding = PaddingValues(16.dp, 12.dp)) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(label, style = MaterialTheme.typography.labelMedium)
                Text(options.firstOrNull { it.first == selected }?.second ?: "Choose an option", style = MaterialTheme.typography.bodyLarge)
            }
            Text("▾", Modifier.padding(start = 12.dp))
        }
        DropdownMenu(expanded = expanded && enabled, onDismissRequest = { expanded = false }) {
            options.forEach { (value, title) ->
                DropdownMenuItem(text = { Text(title) }, onClick = { expanded = false; onChange(value) })
            }
        }
    }
}

@Composable fun BooleanOverride(label: String, override: Boolean?, default: Boolean, enabled: Boolean, onChange: (Boolean?) -> Unit) {
    ChoiceField(label, override?.toString() ?: "default", listOf(
        "default" to "Use defaults (${if (default) "on" else "off"})",
        "true" to "On", "false" to "Off"), enabled) { value ->
        onChange(if (value == "default") null else value.toBooleanStrict())
    }
}
