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
    @Test fun connectionDraftSurvivesScrollingAwayFromItsRepository() {
        val url = "git@github.com:owner/repository.git"
        compose.onNodeWithTag("settings-list").performScrollToNode(hasText("Repository URL (HTTPS or SSH)"))
        compose.onNodeWithText("Repository URL (HTTPS or SSH)").performScrollTo().performTextReplacement(url)
        compose.onNodeWithText("Generate SSH key").performScrollTo().assertIsDisplayed()
        compose.onNodeWithTag("settings-list").performScrollToNode(hasText("SynchroGit"))
        compose.onNodeWithTag("settings-list").performScrollToNode(hasText("Repository URL (HTTPS or SSH)"))
        compose.onNodeWithText("Repository URL (HTTPS or SSH)").performScrollTo()
        compose.onNodeWithText(url).assertExists()
    }
    @Test fun staleServiceMessageDoesNotClaimAStoppedEngineIsRunning() {
        SettingsStore(compose.activity).message = "Continuous synchronization is running"
        compose.activityRule.scenario.recreate()
        compose.onNodeWithText("Continuous sync is stopped").assertIsDisplayed()
        compose.onNodeWithText("Continuous synchronization is running").assertDoesNotExist()
        compose.onNodeWithText("Synchronization is stopped; press Start to resume").assertIsDisplayed()
    }
}
