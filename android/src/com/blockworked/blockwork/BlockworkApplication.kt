package com.blockworked.blockwork

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.os.Handler
import android.os.Looper
import org.json.JSONObject
import org.qtproject.qt.android.bindings.QtApplication
import java.lang.ref.WeakReference
import java.util.concurrent.Executors

class BlockworkApplication : QtApplication() {
    companion object { var instance: BlockworkApplication? = null; private set }
    private val handler = Handler(Looper.getMainLooper())
    private var notifiedPrompt = 0L
    private var notifiedInBackground = 0L
    private var running = false
    var activity = WeakReference<BlockworkActivity>(null)
    val worker = Executors.newSingleThreadExecutor()
    lateinit var schedules: ScheduleManager; private set
    override fun onCreate() {
        super.onCreate()
        instance = this
        NativeRuntime.checked(NativeRuntime.start(filesDir.absolutePath))
        getSystemService(NotificationManager::class.java).createNotificationChannel(
            NotificationChannel("automation", "Workflow activity", NotificationManager.IMPORTANCE_DEFAULT))
        schedules = ScheduleManager(this)
        handler.post(object : Runnable {
            override fun run() {
                try {
                    val active = NativeRuntime.runningCount() > 0
                    AutomationService.current?.showStop(active)
                    if (active != running) { running = active; notifyRun(active) }
                    val actions = NativeRuntime.checked(NativeRuntime.takeActions()) as org.json.JSONArray
                    for (index in 0 until actions.length()) {
                        val action = actions.getJSONObject(index)
                        val service = AutomationService.current
                        if (service == null) NativeRuntime.actionReply(action.getLong("id"), JSONObject().put("ok", false).put("error", "Accessibility service disconnected").toString())
                        else if (action.getJSONObject("operation").getString("kind") == "screenshot") service.capture(action.getLong("id"))
                        else service.enqueue(action)
                    }
                    val pending = NativeRuntime.checked(NativeRuntime.pendingPrompt()) as? JSONObject
                    val id = pending?.optLong("id") ?: 0L
                    if (id != notifiedPrompt) {
                        getSystemService(NotificationManager::class.java).cancel(1)
                        notifiedPrompt = id
                        notifiedInBackground = 0L
                    }
                    if (id != 0L && activity.get() == null && notifiedInBackground != id) {
                        notifyPrompt(); notifiedInBackground = id
                    } else if (activity.get() != null && notifiedInBackground != 0L) {
                        getSystemService(NotificationManager::class.java).cancel(1)
                        notifiedInBackground = 0L
                    }
                    activity.get()?.showPrompt(pending)
                } catch (error: Exception) { android.util.Log.e("Blockwork", "Prompt polling failed", error) }
                handler.postDelayed(this, 100)
            }
        })
    }

    private fun notifyRun(active: Boolean) {
        val manager = getSystemService(NotificationManager::class.java)
        if (!active) { manager.cancel(2); return }
        val stop = PendingIntent.getBroadcast(this, 0, Intent(this, StopReceiver::class.java),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        val open = PendingIntent.getActivity(this, 0, Intent(this, BlockworkActivity::class.java),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        val notification = Notification.Builder(this, "automation")
            .setSmallIcon(android.R.drawable.ic_media_play).setContentTitle("Workflow running")
            .setContentIntent(open).setOngoing(true)
            .addAction(Notification.Action.Builder(null, "Stop", stop).build()).build()
        try { manager.notify(2, notification) } catch (_: SecurityException) {}
    }

    private fun notifyPrompt() {
        val open = PendingIntent.getActivity(this, 0, Intent(this, BlockworkActivity::class.java),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        val stop = PendingIntent.getBroadcast(this, 0, Intent(this, StopReceiver::class.java),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        val notification = Notification.Builder(this, "automation")
            .setSmallIcon(android.R.drawable.ic_dialog_info).setContentTitle("Workflow needs your answer")
            .setContentText("Open Blockwork to continue or stop the run.")
            .setContentIntent(open).addAction(Notification.Action.Builder(null, "Stop", stop).build())
            .setAutoCancel(true).build()
        try { getSystemService(NotificationManager::class.java).notify(1, notification) }
        catch (_: SecurityException) { /* Activity still shows the pending request on resume. */ }
    }
}

class StopReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        (context.applicationContext as BlockworkApplication).worker.execute {
            NativeRuntime.call("emergency_stop", "{}")
        }
    }
}
