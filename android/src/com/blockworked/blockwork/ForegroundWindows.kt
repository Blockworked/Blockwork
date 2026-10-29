package com.blockworked.blockwork

/** Value-only window snapshots; framework node/window handles never escape a scan. */
data class AppWindow(val app: String?, val application: Boolean, val defaultDisplay: Boolean,
    val active: Boolean, val focused: Boolean, val layer: Int)

object ForegroundWindows {
    private val systemOverlays = setOf("android", "com.android.systemui", "com.android.permissioncontroller", "com.google.android.permissioncontroller")
    fun resolve(windows: List<AppWindow>): String? = windows.asSequence()
        .filter { it.application && it.defaultDisplay && !it.app.isNullOrBlank() &&
            it.app !in systemOverlays }
        .maxWithOrNull(compareBy<AppWindow> { it.active }.thenBy { it.focused }.thenBy { it.layer })?.app
}
