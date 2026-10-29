package com.blockworked.blockwork

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.GestureDescription
import android.app.KeyguardManager
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.graphics.Path
import android.graphics.PixelFormat
import android.hardware.display.DisplayManager
import android.os.Bundle
import android.os.PowerManager
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityNodeInfo
import android.view.Gravity
import android.view.Display
import android.view.WindowManager
import android.widget.Button
import org.json.JSONObject
import java.util.ArrayDeque

/** Every action runs on the main looper and resolves nodes afresh. */
class AutomationService : AccessibilityService() {
    private val queued = ArrayDeque<JSONObject>()
    private var active: Long? = null
    @Volatile private var connected = false
    private var stopButton: Button? = null
    private var overlaysHidden = false
    val screenshots by lazy { ScreenshotCoordinator(this) }
    val isReady get() = connected && unlocked
    private val foregroundHandler = android.os.Handler(android.os.Looper.getMainLooper())
    private var lastForeground: String? = null
    private val observeForeground = Runnable {
        try { scanForeground() }
        catch (error: Exception) { resetForeground(); android.util.Log.w("Blockwork", "Could not resolve foreground app", error) }
    }
    fun resetForeground() {
        foregroundHandler.removeCallbacks(observeForeground)
        lastForeground = null
        NativeRuntime.foregroundApp("")
    }
    fun scheduleForegroundObservation() {
        foregroundHandler.removeCallbacks(observeForeground)
        foregroundHandler.postDelayed(observeForeground, 100)
    }
    @Suppress("DEPRECATION")
    private fun scanForeground() {
        if (!isReady || ScreenPicker.isPicking) { resetForeground(); return }
        val snapshots = mutableListOf<AppWindow>()
        for (window in windows) {
            try {
                if (window.type != android.view.accessibility.AccessibilityWindowInfo.TYPE_APPLICATION || window.displayId != Display.DEFAULT_DISPLAY) continue
                val root = window.root ?: continue
                try { snapshots.add(AppWindow(root.packageName?.toString(), true, true, window.isActive, window.isFocused, window.layer)) }
                finally { root.recycle() }
            } finally { window.recycle() }
        }
        val app = ForegroundWindows.resolve(snapshots) ?: return
        if (app != lastForeground) {
            NativeRuntime.checked(NativeRuntime.foregroundApp(app))
            lastForeground = app
        }
    }
    private var lastGeometry: Geometry? = null
    private val displayListener = object : DisplayManager.DisplayListener {
        override fun onDisplayAdded(id: Int) { if (id == Display.DEFAULT_DISPLAY) geometryChanged() }
        override fun onDisplayRemoved(id: Int) { if (id == Display.DEFAULT_DISPLAY) geometryChanged() }
        override fun onDisplayChanged(id: Int) { if (id == Display.DEFAULT_DISPLAY) geometryChanged() }
    }
    data class Geometry(val width: Int, val height: Int, val rotation: Int)
    @Suppress("DEPRECATION")
    fun displayGeometry(): Geometry {
        val display = getSystemService(DisplayManager::class.java).getDisplay(Display.DEFAULT_DISPLAY)
            ?: error("The default display is unavailable")
        val metrics = android.util.DisplayMetrics(); display.getRealMetrics(metrics)
        return Geometry(metrics.widthPixels, metrics.heightPixels, display.rotation)
    }
    private fun geometryChanged() {
        val geometry = runCatching { displayGeometry() }.getOrElse {
            lastGeometry = null
            ScreenPicker.cancel("The default display is unavailable")
            screenshots.invalidate("The default display is unavailable")
            NativeRuntime.displayChanged(0, 0)
            publishReady(false)
            return
        }
        if (lastGeometry == geometry) return
        lastGeometry = geometry
        ScreenPicker.cancel("Display geometry changed; pick again")
        screenshots.invalidate("Display geometry changed; capture again")
        NativeRuntime.displayChanged(geometry.width, geometry.height)
        if (connected) publishReady(unlocked)
    }
    fun hideOverlaysForCapture(hidden: Boolean) {
        overlaysHidden = hidden
        stopButton?.visibility = if (hidden) android.view.View.INVISIBLE else android.view.View.VISIBLE
        ScreenPicker.hideForCapture(hidden)
    }
    fun capture(id: Long) {
        screenshots.request { result ->
            val bitmap = result.getOrNull()
            if (bitmap == null) reply(id, result.exceptionOrNull()?.message ?: "Screen capture failed")
            else (application as BlockworkApplication).worker.execute {
                try {
                    val pixels = IntArray(bitmap.width * bitmap.height)
                    bitmap.getPixels(pixels, 0, bitmap.width, 0, 0, bitmap.width, bitmap.height)
                    NativeRuntime.screenshotReply(id, bitmap.width, bitmap.height, pixels)
                } catch (error: Exception) { reply(id, error.message ?: "Screen capture failed") }
                finally { bitmap.recycle() }
            }
        }
    }
    private val unlocked get() = getSystemService(PowerManager::class.java).isInteractive &&
        !getSystemService(KeyguardManager::class.java).isKeyguardLocked
    private val lockReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            publishReady(connected && unlocked)
            if (!unlocked) {
                resetForeground()
                failAll("Device is locked or not interactive")
                ScreenPicker.cancel("Device is locked or not interactive")
                screenshots.invalidate("Device is locked or not interactive")
            } else scheduleForegroundObservation()
        }
    }
    override fun onServiceConnected() {
        current = this
        connected = true
        registerReceiver(lockReceiver, IntentFilter().apply {
            addAction(Intent.ACTION_SCREEN_OFF); addAction(Intent.ACTION_SCREEN_ON); addAction(Intent.ACTION_USER_PRESENT)
        })
        getSystemService(DisplayManager::class.java).registerDisplayListener(displayListener, android.os.Handler(mainLooper))
        geometryChanged()
        publishReady(unlocked)
        resetForeground()
        scheduleForegroundObservation()
    }
    private fun publishReady(ready: Boolean) {
        // Rust invalidates capture generations without waiting for an active
        // capture; revocation is visible before another action can be queued.
        NativeRuntime.serviceReady(ready)
    }
    override fun onAccessibilityEvent(event: AccessibilityEvent?) {
        if (event?.eventType == AccessibilityEvent.TYPE_WINDOW_STATE_CHANGED || event?.eventType == AccessibilityEvent.TYPE_WINDOWS_CHANGED)
            scheduleForegroundObservation()
    }
    override fun onInterrupt() {
        connected = false
        resetForeground()
        publishReady(false)
        failAll("Accessibility service interrupted")
        ScreenPicker.cancel("Accessibility service interrupted")
        screenshots.invalidate("Accessibility service interrupted")
    }
    override fun onDestroy() {
        connected = false
        resetForeground()
        if (current === this) current = null
        publishReady(false)
        failAll("Accessibility service disconnected")
        ScreenPicker.cancel("Accessibility service disconnected")
        screenshots.invalidate("Accessibility service disconnected", close = true)
        getSystemService(DisplayManager::class.java).unregisterDisplayListener(displayListener)
        showStop(false)
        try { unregisterReceiver(lockReceiver) } catch (_: IllegalArgumentException) {}
        super.onDestroy()
    }
    fun showStop(active: Boolean) {
        val manager = getSystemService(WindowManager::class.java)
        if (!active) { stopButton?.let { manager.removeView(it) }; stopButton = null; return }
        if (stopButton != null) return
        val button = Button(this).apply {
            text = "Stop"; contentDescription = "Stop Blockwork workflows"
            minWidth = (48 * resources.displayMetrics.density).toInt()
            minHeight = minWidth
            setOnClickListener { NativeRuntime.call("emergency_stop", "{}") }
        }
        val layout = WindowManager.LayoutParams(WindowManager.LayoutParams.WRAP_CONTENT, WindowManager.LayoutParams.WRAP_CONTENT,
            WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE, PixelFormat.TRANSLUCENT).apply {
            gravity = Gravity.TOP or Gravity.END
        }
        manager.addView(button, layout)
        stopButton = button
        button.visibility = if (overlaysHidden) android.view.View.INVISIBLE else android.view.View.VISIBLE
    }
    private fun reply(id: Long, error: String? = null, result: Any? = null) {
        NativeRuntime.actionReply(id, JSONObject().put("ok", error == null)
            .put("result", result ?: JSONObject.NULL).put("error", error ?: JSONObject.NULL).toString())
    }
    private fun failAll(error: String) {
        active?.let { reply(it, error) }
        active = null
        while (!queued.isEmpty()) reply(queued.removeFirst().getLong("id"), error)
    }
    fun enqueue(request: JSONObject) {
        val id = request.getLong("id")
        if (request.getJSONObject("operation").getString("kind") == "cancel") {
            queued.removeAll { it.getLong("id") == id }
            // dispatchGesture has no cancel API. Keep an in-flight gesture
            // serialized until its callback; never replay it after stopping.
            return
        }
        queued.addLast(request)
        advance()
    }
    private fun advance() {
        if (active != null || queued.isEmpty()) return
        val request = queued.removeFirst()
        val id = request.getLong("id")
        active = id
        if (!connected || !unlocked) { complete(id, "Enable accessibility and unlock the device"); return }
        try {
            val operation = request.getJSONObject("operation")
            when (operation.getString("kind")) {
                "gesture" -> gesture(id, operation.getJSONObject("gesture"))
                "element" -> complete(id, result = element(operation))
                "launch_app" -> { AndroidApps.launch(this, operation.getString("package")); complete(id) }
                "text" -> { insertText(operation.getString("text")); complete(id) }
                else -> complete(id, "Unknown Android operation")
            }
        } catch (error: Exception) { complete(id, error.message ?: "Android action failed") }
    }
    private fun complete(id: Long, error: String? = null, result: Any? = null) {
        if (active != id) return
        reply(id, error, result)
        active = null
        advance()
    }
    private fun gesture(id: Long, gesture: JSONObject) {
        val action = gesture.getString("action")
        val global = when (action) {
            "Back" -> GLOBAL_ACTION_BACK
            "Home" -> GLOBAL_ACTION_HOME
            "Recents" -> GLOBAL_ACTION_RECENTS
            else -> null
        }
        if (global != null) {
            complete(id, if (performGlobalAction(global)) null else "Android rejected the navigation action")
            return
        }
        require(action in arrayOf("Tap", "LongPress", "Swipe")) { "Unknown touch action" }
        val metrics = android.util.DisplayMetrics()
        @Suppress("DEPRECATION")
        getSystemService(DisplayManager::class.java).getDisplay(Display.DEFAULT_DISPLAY).getRealMetrics(metrics)
        fun coordinate(key: String, limit: Int): Float {
            val n = gesture.getDouble(key)
            require(n.isFinite() && n >= 0 && n < limit) { "Gesture point is outside the default display" }
            return n.toFloat()
        }
        val x = coordinate("x", metrics.widthPixels)
        val y = coordinate("y", metrics.heightPixels)
        val path = Path().apply {
            moveTo(x, y)
            if (action == "Swipe") lineTo(coordinate("end_x", metrics.widthPixels), coordinate("end_y", metrics.heightPixels))
        }
        val requestedDuration = gesture.getLong("duration_ms")
        val duration = if (action == "LongPress") maxOf(requestedDuration, android.view.ViewConfiguration.getLongPressTimeout().toLong()) else requestedDuration
        require(duration in 1..60_000) { "Gesture duration must be 1–60000 ms" }
        val description = GestureDescription.Builder()
            .addStroke(GestureDescription.StrokeDescription(path, 0, duration)).build()
        val accepted = dispatchGesture(description, object : GestureResultCallback() {
            override fun onCompleted(gestureDescription: GestureDescription) { complete(id) }
            override fun onCancelled(gestureDescription: GestureDescription) { complete(id, "Android cancelled the gesture") }
        }, null)
        if (!accepted) complete(id, "Android rejected the gesture; check accessibility access")
    }
    fun validatePickedElement(selector: JSONObject) {
        element(JSONObject().put("selector", selector).put("action", "Validate"))
    }
    @Suppress("DEPRECATION")
    private fun element(operation: JSONObject): Any? {
        val supplied = operation.getJSONObject("selector")
        val selector = ElementSelector(supplied.getString("package"), supplied.optString("resource_id"),
            supplied.optString("text"), supplied.optString("description"), supplied.optString("class"))
        val action = operation.getString("action")
        val queue = ArrayDeque<AccessibilityNodeInfo>()
        val matches = mutableListOf<AccessibilityNodeInfo>()
        // Only application windows participate; the Stop overlay and system
        // keyboard must never become selector candidates.
        val seenWindows = mutableSetOf<Int>()
        var visited = 0
        try {
        for (window in windows) {
            try {
                if (window.type == android.view.accessibility.AccessibilityWindowInfo.TYPE_APPLICATION && seenWindows.add(window.id))
                    window.root?.let { if (it.packageName?.toString() == selector.app) queue.add(it) else it.recycle() }
            } finally { window.recycle() }
        }
            while (!queue.isEmpty()) {
                val node = queue.removeFirst()
                try {
                    require(++visited <= 10000) { "Accessibility tree is too large; narrow the target app" }
                    if (selector.matches(node.packageName?.toString(), node.viewIdResourceName,
                        node.text?.toString(), node.contentDescription?.toString(), node.className?.toString())) {
                        matches.add(AccessibilityNodeInfo.obtain(node))
                        if (action == "Exists") return true
                        selector.requireUnique(matches.size)
                    }
                    for (index in 0 until node.childCount) {
                        require(visited + queue.size < 10000) { "Accessibility tree is too large; narrow the target app" }
                        node.getChild(index)?.let { queue.add(it) }
                    }
                } finally { node.recycle() }
            }
            if (action == "Exists") return false
            selector.requireUnique(matches.size)
            val node = matches.single()
            require(node.refresh()) { "Element changed while resolving; try again" }
            require(selector.matches(node.packageName?.toString(), node.viewIdResourceName,
                node.text?.toString(), node.contentDescription?.toString(), node.className?.toString())) { "Element changed while resolving; try again" }
            if (action == "Validate") return null
            if (action == "ReadText") {
                require(!node.isPassword && node.text != null) { "This element does not expose readable text" }
                return node.text.toString()
            }
            val nativeAction = when (action) {
                "Click" -> AccessibilityNodeInfo.ACTION_CLICK
                "LongClick" -> AccessibilityNodeInfo.ACTION_LONG_CLICK
                "ScrollForward" -> AccessibilityNodeInfo.ACTION_SCROLL_FORWARD
                "ScrollBackward" -> AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD
                "SetText" -> AccessibilityNodeInfo.ACTION_SET_TEXT
                else -> throw IllegalArgumentException("Unknown element action")
            }
            require(node.isEnabled) { "Element is disabled" }
            require(node.actionList.any { it.id == nativeAction }) { "This element does not support the requested action" }
            val arguments = if (action == "SetText") Bundle().apply {
                putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, operation.getString("text"))
            } else null
            require(node.performAction(nativeAction, arguments)) { "Element rejected the action; it may have changed" }
            return null
        } finally {
            queue.forEach { it.recycle() }
            matches.forEach { it.recycle() }
        }
    }
    @Suppress("DEPRECATION")
    private fun insertText(text: String) {
        val node = rootInActiveWindow?.findFocus(AccessibilityNodeInfo.FOCUS_INPUT)
            ?: throw IllegalStateException("Focus an editable text field first")
        try {
            require(node.isEditable && node.text != null && !node.isPassword) { "This control does not expose editable text for insertion. Choose another text field." }
            val old = node.text.toString()
            val start = node.textSelectionStart
            val end = node.textSelectionEnd
            require(start in 0..old.length && end in start..old.length) { "This control does not expose its text selection. Choose another text field." }
            val replacement = old.replaceRange(start, end, text)
            val ok = node.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, Bundle().apply {
                putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, replacement)
            })
            require(ok) { "This control rejected text insertion" }
            val cursor = start + text.length
            require(node.performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION, Bundle().apply {
                putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, cursor)
                putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, cursor)
            })) { "Text was inserted, but this control cannot restore the cursor" }
        } finally { node.recycle() }
    }
    companion object { var current: AutomationService? = null; private set }
}
