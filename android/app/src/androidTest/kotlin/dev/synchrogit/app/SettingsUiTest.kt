package dev.synchrogit.app

import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import org.junit.Rule
import org.junit.Test

class SettingsUiTest {
    @get:Rule val compose = createAndroidComposeRule<MainActivity>()
    @Test fun settingsScreenLoadsAndShowsSyncControls() {
        compose.onNodeWithText("SynchroGit").assertIsDisplayed()
        compose.onNodeWithText("Start").assertIsDisplayed()
        compose.onNodeWithText("Folder access").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("Pull interval").performScrollTo().assertExists()
    }
}
