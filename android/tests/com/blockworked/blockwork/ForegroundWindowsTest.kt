package com.blockworked.blockwork

import org.junit.Assert.*
import org.junit.Test

class ForegroundWindowsTest {
    private fun app(name: String, layer: Int = 1, active: Boolean = true, focused: Boolean = true) = AppWindow(name, true, true, active, focused, layer)
    @Test fun overlaysAndKeyboardDoNotReplaceTheApplication() {
        val target = app("fixture.app")
        assertEquals("fixture.app", ForegroundWindows.resolve(listOf(target,
            AppWindow("keyboard.app", false, true, true, true, 90),
            AppWindow("com.blockworked.blockwork", false, true, true, true, 100),
            app("com.android.systemui", 101), app("com.google.android.permissioncontroller", 102))))
    }
    @Test fun splitScreenPrefersTheActiveAppAndIgnoresOtherDisplays() {
        assertEquals("fixture.left", ForegroundWindows.resolve(listOf(app("fixture.right", 10, false, false), app("fixture.left", 2),
            AppWindow("fixture.external", true, false, true, true, 100))))
    }
    @Test fun homeAndEditorAreRealApplicationsAndMissingWindowsAreNotTransitions() {
        assertEquals("launcher.app", ForegroundWindows.resolve(listOf(app("fixture.app", 0, false, false), app("launcher.app", 2))))
        assertEquals("com.blockworked.blockwork", ForegroundWindows.resolve(listOf(app("com.blockworked.blockwork"))))
        assertNull(ForegroundWindows.resolve(listOf(AppWindow("keyboard.app", false, true, true, true, 10))))
        assertNull(ForegroundWindows.resolve(emptyList()))
    }
    @Test fun launchPackagesCannotContainCommandsComponentsOrUris() {
        listOf("com.example.app", "android", "com.example.App_2").forEach { AndroidApps.validate(it) }
        for (invalid in listOf("", " com.example.app", "com.app/.Main", "intent://app", "com.app;echo bad", "com..app", "com.1app")) {
            try { AndroidApps.validate(invalid); fail("Accepted invalid package $invalid") } catch (_: IllegalArgumentException) {}
        }
    }
}
