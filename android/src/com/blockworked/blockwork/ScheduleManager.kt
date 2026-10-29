package com.blockworked.blockwork

import android.app.AlarmManager
import android.app.Notification
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.PowerManager
import org.json.JSONObject
import java.util.TimeZone
import java.util.concurrent.Executors

/** The OS owns delivery; polling only reconciles edits and observed permissions. */
class ScheduleManager(private val app: BlockworkApplication) {
    private val executor = Executors.newSingleThreadExecutor()
    private val handler = Handler(Looper.getMainLooper())
    private val manager = app.getSystemService(AlarmManager::class.java)
    private val preferences = app.getSharedPreferences("android-alarms", Context.MODE_PRIVATE)
    private val installed = mutableMapOf<String, String>()
    private var pending = false
    @Volatile private var deliveryPending = false
    private var lastStatus: String? = null
    init {
        // PendingIntent identities persist across process death. On restart
        // re-arm every desired occurrence, and cancel stale identities.
        for (token in preferences.getStringSet("tokens", emptySet()).orEmpty()) installed[token] = ""
        handler.postDelayed(object : Runnable {
            override fun run() { refresh(); handler.postDelayed(this, 1000) }
        }, 1500)
    }
    private fun exactAccess() = Build.VERSION.SDK_INT < 31 || manager.canScheduleExactAlarms()
    @Synchronized fun refresh(delayMs: Long = 0) {
        if (pending || deliveryPending) return
        pending = true
        handler.postDelayed({ executor.execute {
            try {
                val plan = NativeRuntime.checked(NativeRuntime.alarmPlan(System.currentTimeMillis(), TimeZone.getDefault().id, exactAccess())) as JSONObject
                val desired = plan.getJSONArray("alarms")
                val retained = mutableSetOf<String>()
                val problems = mutableListOf<String>()
                val errors = plan.getJSONArray("errors")
                for (index in 0 until errors.length()) problems.add(errors.getString(index))
                for (index in 0 until desired.length()) {
                    val alarm = desired.getJSONObject(index)
                    val token = alarm.getString("token")
                    require(AlarmPolicy.validToken(token)) { "Invalid native alarm token" }
                    retained.add(token)
                    val signature = alarm.toString()
                    if (installed[token] == signature) continue
                    val intent = intent(token)
                    val time = alarm.getLong("at_ms").coerceAtLeast(System.currentTimeMillis() + 100)
                    try {
                        when (AlarmPolicy.mode(alarm.getBoolean("exact"), exactAccess())) {
                            AlarmMode.EXACT -> manager.setExactAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, time, intent)
                            AlarmMode.BEST_EFFORT -> manager.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, time, intent)
                            AlarmMode.BLOCKED -> { problems.add("Exact-time schedule is blocked: allow exact alarms in Android settings"); manager.cancel(intent); continue }
                        }
                        installed[token] = signature
                    } catch (error: SecurityException) {
                        manager.cancel(intent); installed.remove(token)
                        problems.add("Android denied the requested alarm; check exact-alarm access")
                    }
                }
                for (token in installed.keys.toList()) if (token !in retained) { cancel(token); installed.remove(token) }
                check(preferences.edit().putStringSet("tokens", installed.keys.toSet()).commit()) { "Could not persist Android alarm identities" }
                publish(problems.distinct().take(3).joinToString("\n"), plan.optBoolean("paused"))
            } catch (error: Exception) {
                // An unreadable ledger/time zone must not leave stale alarms
                // capable of launching a previous workflow definition.
                for (token in installed.keys.toList()) cancel(token)
                installed.clear()
                preferences.edit().putStringSet("tokens", emptySet()).commit()
                publish(error.message ?: "Could not reconcile Android schedules")
                android.util.Log.e("Blockwork", "Alarm reconciliation failed", error)
            } finally { synchronized(this) { pending = false } }
        } }, delayMs)
    }
    fun deliver(token: String, completed: () -> Unit) {
        if (!AlarmPolicy.validToken(token)) { completed(); return }
        deliveryPending = true
        // Accessibility may reconnect just after Application creation when an
        // alarm wakes a dead process. Stay within the receiver's time budget.
        handler.postDelayed({
            NativeRuntime.scheduleUiReady(AutomationService.current?.isReady == true && !ScreenPicker.isPicking)
            executor.execute {
            try {
                installed.remove(token)
                val fired = NativeRuntime.checked(NativeRuntime.deliverAlarm(token, System.currentTimeMillis(), TimeZone.getDefault().id, exactAccess())) == true
                if (!fired) completed()
                else {
                val deadline = android.os.SystemClock.uptimeMillis() + 7000
                handler.postDelayed(object : Runnable {
                    override fun run() {
                        if (NativeRuntime.runningCount() == 0 || android.os.SystemClock.uptimeMillis() >= deadline) completed()
                        else handler.postDelayed(this, 100)
                    }
                }, 200)
                }
            } catch (error: Exception) { publish(error.message ?: "Alarm delivery failed"); completed() }
            finally { deliveryPending = false; refresh() }
        } }, 1000)
    }
    private fun intent(token: String) = PendingIntent.getBroadcast(app, 0,
        Intent(app, AlarmReceiver::class.java).setAction("com.blockworked.blockwork.ALARM")
            .setData(Uri.Builder().scheme("blockwork-alarm").authority("occurrence").appendPath(token).build()),
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
    private fun cancel(token: String) {
        val intent = intent(token); manager.cancel(intent); intent.cancel()
    }
    private fun publish(problem: String, paused: Boolean = false) {
        status = if (problem.isEmpty() && paused) "Schedules paused • use Resume triggers to re-arm" else if (problem.isEmpty()) "Android schedules ready • best-effort timing unless Exact time is enabled" else problem
        if (problem == lastStatus) return
        lastStatus = problem
        val notifications = app.getSystemService(NotificationManager::class.java)
        if (problem.isEmpty()) { notifications.cancel(3); return }
        val open = PendingIntent.getActivity(app, 3, Intent(app, BlockworkActivity::class.java), PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        try { notifications.notify(3, Notification.Builder(app, "automation").setSmallIcon(android.R.drawable.ic_dialog_info)
            .setContentTitle("Schedule needs attention").setContentText(problem).setStyle(Notification.BigTextStyle().bigText(problem)).setContentIntent(open).setAutoCancel(true).build()) }
        catch (_: SecurityException) {}
    }
    companion object {
        @Volatile private var status = "Checking Android schedules…"
        @JvmStatic fun currentStatus(): String = status
    }
}

class AlarmReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val token = intent.data?.lastPathSegment ?: return
        if (!AlarmPolicy.validToken(token)) return
        val pending = goAsync()
        val wake = context.getSystemService(PowerManager::class.java).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "Blockwork:alarm-delivery")
        wake.acquire(9000)
        val handler = Handler(Looper.getMainLooper())
        var finished = false
        fun finish() { handler.post {
            if (!finished) { finished = true; if (wake.isHeld) wake.release(); pending.finish() }
        } }
        handler.postDelayed({ finish() }, 8500)
        (context.applicationContext as BlockworkApplication).schedules.deliver(token) { finish() }
    }
}

class RescheduleReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        // Java and native time-zone caches must observe the new zone before
        // computing wall-clock recurrences.
        (context.applicationContext as BlockworkApplication).schedules.refresh(1500)
    }
}
