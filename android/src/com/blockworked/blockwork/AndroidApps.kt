package com.blockworked.blockwork

import android.content.Context
import android.content.Intent
import org.json.JSONArray
import org.json.JSONObject

object AndroidApps {
    private val packagePattern = Regex("[A-Za-z_][A-Za-z0-9_]*(\\.[A-Za-z_][A-Za-z0-9_]*)*")
    @Volatile private var pending: String? = null
    fun validate(packageName: String) {
        require(packageName.length <= 255 && packagePattern.matches(packageName)) {
            "Choose an Android app package, such as com.example.app"
        }
    }
    fun launch(context: Context, packageName: String) {
        validate(packageName)
        val intent = context.packageManager.getLaunchIntentForPackage(packageName)
            ?: error("This package has no available launcher activity; choose an installed app in this profile")
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        try { context.startActivity(intent) }
        catch (error: SecurityException) { throw IllegalStateException("Android denied the app launch; check the app and accessibility access", error) }
    }
    @JvmStatic fun requestInstalledApps() {
        val app = BlockworkApplication.instance ?: return
        app.worker.execute {
            val result = try {
                val query = Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER)
                @Suppress("DEPRECATION")
                val activities = app.packageManager.queryIntentActivities(query, 0)
                val entries = activities.filter { it.activityInfo.enabled && it.activityInfo.exported }
                    .distinctBy { it.activityInfo.packageName }
                    .map { Pair(it.activityInfo.packageName, it.loadLabel(app.packageManager).toString()) }
                    .sortedBy { it.second.lowercase() }
                val list = JSONArray()
                for ((id, name) in entries) list.put(JSONObject().put("command", id).put("name", name).put("icon", JSONObject.NULL))
                JSONObject().put("ok", true).put("result", list)
            } catch (error: Exception) { JSONObject().put("ok", false).put("error", error.message ?: "Could not list launchable apps") }
            synchronized(this) { pending = result.toString() }
        }
    }
    @JvmStatic @Synchronized fun takeInstalledApps(): String {
        val value = pending ?: return ""
        pending = null
        return value
    }
}
