package dev.synchrogit.app

import android.content.res.Configuration
import android.os.Build
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test

class SettingsControlsTest {
    @get:Rule val compose = createComposeRule()

    @Test fun themeTracksSystemDarkModeAndDynamicColors() {
        var night by mutableStateOf(false)
        var actual: ColorScheme? = null
        var expected: ColorScheme? = null
        compose.setContent {
            val context = LocalContext.current
            val configuration = Configuration(LocalConfiguration.current).apply {
                uiMode = (uiMode and Configuration.UI_MODE_NIGHT_MASK.inv()) or
                    if (night) Configuration.UI_MODE_NIGHT_YES else Configuration.UI_MODE_NIGHT_NO
            }
            expected = when {
                Build.VERSION.SDK_INT >= 31 && night -> dynamicDarkColorScheme(context)
                Build.VERSION.SDK_INT >= 31 -> dynamicLightColorScheme(context)
                night -> darkColorScheme()
                else -> lightColorScheme()
            }
            CompositionLocalProvider(LocalConfiguration provides configuration) {
                SynchroGitTheme { actual = MaterialTheme.colorScheme }
            }
        }
        compose.runOnIdle {
            assertEquals(expected!!.primary, actual!!.primary)
            assertEquals(expected!!.surface, actual!!.surface)
            night = true
        }
        compose.runOnIdle {
            assertEquals(expected!!.primary, actual!!.primary)
            assertEquals(expected!!.surface, actual!!.surface)
        }
    }

    @Test fun overrideCanInheritFalseAndReturnToInheritance() {
        var override by mutableStateOf<Boolean?>(null)
        var default by mutableStateOf(false)
        compose.setContent { SynchroGitTheme {
            BooleanOverride("Pull override", override, default, true) { override = it }
        } }
        compose.onNodeWithText("Use defaults (off)").assertExists()
        compose.onNodeWithTag("Pull override").performClick()
        compose.onNodeWithText("On").performClick()
        compose.runOnIdle { assertEquals(true, override) }
        compose.onNodeWithTag("Pull override").performClick()
        compose.onNodeWithText("Use defaults (off)").performClick()
        compose.runOnIdle { assertNull(override); default = true }
        compose.onNodeWithText("Use defaults (on)").assertExists()
    }

    @Test fun longSwitchLabelFitsAndWholeRowTogglesAtLargeFont() {
        var checked by mutableStateOf(false)
        val label = "Scheduled sync (15 minutes or later)"
        compose.setContent {
            val density = LocalDensity.current
            CompositionLocalProvider(LocalDensity provides Density(density.density, 1.8f)) {
                SynchroGitTheme { Box(Modifier.width(280.dp)) { Toggle(label, checked, true) { checked = it } } }
            }
        }
        compose.onNodeWithText(label).assertIsDisplayed().assertIsOff().performClick().assertIsOn()
        compose.runOnIdle { assertTrue(checked) }
    }
}
