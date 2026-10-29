package com.blockworked.blockwork

import android.content.Intent
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.PixelFormat
import android.graphics.Rect
import android.graphics.RectF
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.WindowManager
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityWindowInfo
import android.widget.Button
import android.widget.LinearLayout
import android.widget.TextView
import org.json.JSONObject
import java.io.ByteArrayOutputStream
import java.util.ArrayDeque

/** Only main-looper code owns views/nodes. Results contain values, never handles. */
object ScreenPicker {
    @Volatile private var active: Session? = null
    val isPicking get() = active != null
    @Volatile private var result: String? = null
    @JvmStatic fun begin(mode: String, hint: String) {
        val app = BlockworkApplication.instance ?: return
        android.os.Handler(app.mainLooper).post {
            if (active != null) return@post
            result = null
            val service = AutomationService.current
            if (service == null || !service.isReady) {
                result = JSONObject().put("error", "Enable accessibility and unlock the device before picking").toString()
                return@post
            }
            try {
                require(NativeRuntime.runningCount() == 0) { "Stop running workflows before opening the picker" }
                require(mode in listOf("point", "region", "element")) { "Unknown picker mode" }
                val session = Session(service, mode, hint)
                active = session
                NativeRuntime.scheduleUiReady(false)
                service.resetForeground()
                session.start()
            } catch (error: Exception) {
                if (active != null) cancel(error.message ?: "Could not open screen picker")
                else result = JSONObject().put("error", error.message ?: "Could not open screen picker").toString()
            }
        }
    }
    @JvmStatic @Synchronized fun takeResult(): String {
        val pending = result ?: return ""
        result = null
        return pending
    }
    fun hideForCapture(hidden: Boolean) { active?.hide(hidden) }
    fun cancel(reason: String? = null) { active?.finish(if (reason == null) JSONObject().put("canceled", true) else JSONObject().put("error", reason)) }

    private class Session(val service: AutomationService, val mode: String, val hint: String) {
        private val manager = service.getSystemService(WindowManager::class.java)
        private val geometry = service.displayGeometry()
        private val density = service.resources.displayMetrics.density
        private var controls: LinearLayout? = null
        private var surface: SelectionView? = null
        private var frame: Bitmap? = null
        private var rect: PickRect? = null
        private var selector: JSONObject? = null
        private var label: TextView? = null
        private var confirm: Button? = null
        private var bottom = true
        private var finishing = false
        private var captureHidden = false

        private fun params(full: Boolean) = WindowManager.LayoutParams(
            if (full) WindowManager.LayoutParams.MATCH_PARENT else WindowManager.LayoutParams.WRAP_CONTENT,
            if (full) WindowManager.LayoutParams.MATCH_PARENT else WindowManager.LayoutParams.WRAP_CONTENT,
            WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL or
                WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN or
                (if (full) WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS else 0), PixelFormat.TRANSLUCENT).apply {
            gravity = if (full) Gravity.TOP or Gravity.START else (if (bottom) Gravity.BOTTOM else Gravity.TOP) or Gravity.CENTER_HORIZONTAL
            layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS
            if (full) setFitInsetsTypes(0)
            else { y = (32 * density).toInt() }
        }
        private fun button(title: String, action: () -> Unit) = Button(service).apply {
            text = title; minHeight = (48 * density).toInt(); minWidth = (48 * density).toInt()
            setOnClickListener { action() }
        }
        fun start() {
            // The dock leaves other apps interactive until Select is tapped.
            dock(false)
            (service.application as BlockworkApplication).activity.get()?.moveTaskToBack(true)
        }
        private fun dock(selecting: Boolean) {
            controls?.let { runCatching { manager.removeView(it) } }
            val panel = LinearLayout(service).apply {
                orientation = LinearLayout.VERTICAL
                setPadding((8 * density).toInt(), 0, (8 * density).toInt(), 0)
                setBackgroundColor(Color.rgb(32, 34, 40))
            }
            label = TextView(service).apply {
                setTextColor(Color.WHITE); textSize = 14f
                maxWidth = (geometry.width - 32 * density).toInt().coerceAtLeast(120)
                maxLines = 3; ellipsize = android.text.TextUtils.TruncateAt.END
                text = if (selecting) "Choose $mode • ${geometry.width} × ${geometry.height} px" else "Switch to the target app, then tap Select"
                setPadding(0, (8 * density).toInt(), 0, 0)
            }
            panel.addView(label)
            val row = LinearLayout(service)
            if (selecting) {
                confirm = button("Confirm") { accept() }.apply { isEnabled = false }
                row.addView(confirm)
                row.addView(button("Retry") { reset() })
            } else row.addView(button("Select") { select() })
            row.addView(button("Move") {
                bottom = !bottom
                manager.updateViewLayout(panel, params(false))
            })
            row.addView(button("Cancel") { finish(JSONObject().put("canceled", true)) })
            panel.addView(row)
            controls = panel
            manager.addView(panel, params(false))
            hide(captureHidden)
        }
        fun hide(hidden: Boolean) {
            captureHidden = hidden
            controls?.visibility = if (hidden) View.INVISIBLE else View.VISIBLE
            surface?.visibility = if (hidden) View.INVISIBLE else View.VISIBLE
        }
        private fun select() {
            if (finishing) return
            controls?.let { runCatching { manager.removeView(it) } }; controls = null
            if (mode == "element") showSelection(null)
            else service.screenshots.request { capture ->
                if (active !== this || finishing) { capture.getOrNull()?.recycle(); return@request }
                capture.fold({ showSelection(it) }, { finish(JSONObject().put("error", it.message ?: "Capture failed")) })
            }
        }
        private fun showSelection(bitmap: Bitmap?) {
            if (geometry != service.displayGeometry()) { bitmap?.recycle(); finish(JSONObject().put("error", "Display geometry changed; pick again")); return }
            frame = bitmap
            surface = SelectionView().also { manager.addView(it, params(true)) }
            dock(true)
        }
        private fun reset() {
            surface?.let { runCatching { manager.removeView(it) } }; surface = null
            frame?.recycle(); frame = null
            rect = null; selector = null; confirm = null
            dock(false)
        }
        private fun update(x0: Float, y0: Float, x1: Float, y1: Float) {
            rect = if (mode == "region") PickRect.region(x0, y0, x1, y1, geometry.width, geometry.height)
                else PickRect.point(x1, y1, geometry.width, geometry.height)
            selector = null
            if (mode == "element") {
                try {
                    val chosen = elementAt(rect!!.x, rect!!.y)
                    rect = chosen.first; selector = chosen.second
                    label?.text = selector.toString()
                } catch (error: Exception) { label?.text = error.message ?: "No accessible element here"; rect = null }
            } else rect?.let { label?.text = "x ${it.x}  y ${it.y}  ${it.width} × ${it.height} • ${geometry.width} × ${geometry.height} px" + if (mode == "region") " • Drag corners to resize" else "" }
            confirm?.isEnabled = rect != null && (mode != "element" || selector != null)
            surface?.invalidate()
        }
        private fun accept() {
            val chosen = rect ?: return
            if (finishing) return
            if (geometry != service.displayGeometry() || !service.isReady) { finish(JSONObject().put("error", "Display or accessibility access changed; pick again")); return }
            if (mode == "element") {
                val value = selector ?: return
                // Validate uniqueness against a fresh tree before returning.
                try { service.validatePickedElement(value) }
                catch (error: Exception) { label?.text = error.message; return }
                finish(JSONObject().put("selector", value).put("display_width", geometry.width).put("display_height", geometry.height))
                return
            }
            finishing = true
            surface?.let { runCatching { manager.removeView(it) } }; surface = null
            controls?.let { runCatching { manager.removeView(it) } }; controls = null
            frame?.recycle(); frame = null
            // Capture the confirmed live target, with all Blockwork overlays
            // hidden, before returning to the Qt editor.
            service.screenshots.request { capture ->
                val bitmap = capture.getOrNull()
                if (active !== this) { bitmap?.recycle(); return@request }
                if (bitmap == null) { finish(JSONObject().put("error", capture.exceptionOrNull()?.message ?: "Capture failed")); return@request }
                (service.application as BlockworkApplication).worker.execute {
                    val answer = try {
                        require(bitmap.width == geometry.width && bitmap.height == geometry.height) { "Screenshot size differs from display geometry" }
                        val color = bitmap.getPixel(chosen.x + chosen.width / 2, chosen.y + chosen.height / 2)
                        val image = if (chosen.width > 0 && chosen.height > 0) {
                            val crop = Bitmap.createBitmap(bitmap, chosen.x, chosen.y, chosen.width, chosen.height)
                            try {
                                val bytes = ByteArrayOutputStream()
                                require(crop.compress(Bitmap.CompressFormat.PNG, 100, bytes)) { "Image encoding failed" }
                                "data:image/png;base64," + android.util.Base64.encodeToString(bytes.toByteArray(), android.util.Base64.NO_WRAP)
                            } finally { if (crop !== bitmap) crop.recycle() }
                        } else ""
                        JSONObject().put("x", chosen.x).put("y", chosen.y).put("width", chosen.width).put("height", chosen.height)
                            .put("image", image).put("color", String.format("#%06x", color and 0xffffff))
                            .put("display_width", geometry.width).put("display_height", geometry.height)
                    } catch (error: Exception) { JSONObject().put("error", error.message ?: "Capture failed") }
                    finally { bitmap.recycle() }
                    android.os.Handler(service.mainLooper).post { if (active === this) finish(answer) }
                }
            }
        }
        fun finish(answer: JSONObject) {
            if (active !== this) return
            active = null
            NativeRuntime.scheduleUiReady(service.isReady)
            service.resetForeground()
            service.scheduleForegroundObservation()
            surface?.let { runCatching { manager.removeView(it) } }; surface = null
            controls?.let { runCatching { manager.removeView(it) } }; controls = null
            frame?.recycle(); frame = null
            synchronized(ScreenPicker) { result = answer.toString() }
            // Results already contain pixels/selectors. Foregrounding the
            // editor can no longer change what the user confirmed.
            runCatching {
                service.startActivity(Intent(service, BlockworkActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_REORDER_TO_FRONT))
            }.onFailure { android.util.Log.e("Blockwork", "Open Blockwork to collect the picker result", it) }
        }
        @Suppress("DEPRECATION")
        private fun elementAt(x: Int, y: Int): Pair<PickRect, JSONObject> {
            val queue = ArrayDeque<AccessibilityNodeInfo>()
            var chosen: Pair<PickRect, JSONObject>? = null
            var smallest = Long.MAX_VALUE
            var visited = 0
            try {
                // Frontmost application window wins. System and keyboard
                // windows, including Blockwork's overlays, never participate.
                val candidates = service.windows
                try { for (window in candidates.sortedByDescending { it.layer }) {
                        val bounds = Rect(); window.getBoundsInScreen(bounds)
                        if (window.type == AccessibilityWindowInfo.TYPE_APPLICATION && bounds.contains(x, y) && queue.isEmpty()) {
                            window.root?.let { if (it.packageName?.toString() != service.packageName) queue.add(it) else it.recycle() }
                            if (queue.isNotEmpty()) break
                        }
                } } finally { candidates.forEach { it.recycle() } }
                while (queue.isNotEmpty()) {
                    val node = queue.removeFirst()
                    try {
                        require(++visited <= 10000) { "Accessibility tree is too large" }
                        val bounds = Rect(); node.getBoundsInScreen(bounds)
                        if (node.isVisibleToUser && bounds.contains(x, y)) {
                            val area = bounds.width().toLong() * bounds.height()
                            val app = node.packageName?.toString().orEmpty()
                            val id = node.viewIdResourceName.orEmpty()
                            val text = if (node.isPassword || node.isEditable) "" else node.text?.toString().orEmpty()
                            val description = node.contentDescription?.toString().orEmpty()
                            val klass = node.className?.toString().orEmpty()
                            if (area <= smallest && app.isNotBlank() && listOf(id, text, description, klass).any { it.isNotEmpty() }) {
                                val fields = JSONObject().put("package", app).put("resource_id", id)
                                    .put("text", text)
                                    .put("description", description).put("class", klass)
                                chosen = Pair(PickRect(bounds.left.coerceAtLeast(0), bounds.top.coerceAtLeast(0),
                                    (bounds.right.coerceAtMost(geometry.width) - bounds.left.coerceAtLeast(0)).coerceAtLeast(1),
                                    (bounds.bottom.coerceAtMost(geometry.height) - bounds.top.coerceAtLeast(0)).coerceAtLeast(1)), fields)
                                smallest = area
                            }
                        }
                        for (index in 0 until node.childCount) {
                            require(visited + queue.size < 10000) { "Accessibility tree is too large" }
                            node.getChild(index)?.let { queue.add(it) }
                        }
                    } finally { node.recycle() }
                }
            } finally { queue.forEach { it.recycle() } }
            return chosen ?: error("No accessible element here; try another point")
        }
        private inner class SelectionView : View(service) {
            private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
            private var startX = 0f; private var startY = 0f
            private var fingerX = 0f; private var fingerY = 0f
            override fun onTouchEvent(event: MotionEvent): Boolean {
                if (event.pointerCount != 1) return true
                when (event.actionMasked) {
                    MotionEvent.ACTION_DOWN -> {
                        startX = event.rawX; startY = event.rawY
                        // Drag a selected corner to resize; drag elsewhere to redraw.
                        val prior = rect
                        if (mode == "region" && prior != null) {
                            val nearLeft = kotlin.math.abs(startX - prior.x) < 24 * density
                            val nearRight = kotlin.math.abs(startX - prior.x - prior.width) < 24 * density
                            val nearTop = kotlin.math.abs(startY - prior.y) < 24 * density
                            val nearBottom = kotlin.math.abs(startY - prior.y - prior.height) < 24 * density
                            if ((nearLeft || nearRight) && (nearTop || nearBottom)) {
                                startX = (if (nearLeft) prior.x + prior.width else prior.x).toFloat()
                                startY = (if (nearTop) prior.y + prior.height else prior.y).toFloat()
                            }
                        }
                    }
                    MotionEvent.ACTION_MOVE, MotionEvent.ACTION_UP -> {}
                    else -> return true
                }
                fingerX = event.rawX; fingerY = event.rawY
                update(startX, startY, fingerX, fingerY)
                return true
            }
            override fun onDraw(canvas: Canvas) {
                super.onDraw(canvas)
                val location = IntArray(2); getLocationOnScreen(location)
                canvas.save(); canvas.translate(-location[0].toFloat(), -location[1].toFloat())
                paint.color = Color.argb(35, 0, 0, 0); paint.style = Paint.Style.FILL
                canvas.drawRect(0f, 0f, geometry.width.toFloat(), geometry.height.toFloat(), paint)
                rect?.let {
                    paint.color = Color.CYAN; paint.style = Paint.Style.STROKE; paint.strokeWidth = 2 * density
                    if (it.width > 0) {
                        canvas.drawRect(it.x.toFloat(), it.y.toFloat(), (it.x + it.width).toFloat(), (it.y + it.height).toFloat(), paint)
                        if (mode == "region") for (x in listOf(it.x, it.x + it.width)) for (y in listOf(it.y, it.y + it.height))
                            canvas.drawCircle(x.toFloat(), y.toFloat(), 5 * density, paint)
                    } else { canvas.drawLine(it.x - 16f, it.y.toFloat(), it.x + 16f, it.y.toFloat(), paint); canvas.drawLine(it.x.toFloat(), it.y - 16f, it.x.toFloat(), it.y + 16f, paint) }
                }
                frame?.let { bitmap ->
                    if (rect != null) {
                        val size = (104 * density).toInt().coerceAtMost(geometry.width / 2)
                        val left = if (fingerX > geometry.width / 2) 16 * density else geometry.width - size - 16 * density
                        val top = if (this@Session.bottom) 48 * density else geometry.height - size - 64 * density
                        val target = RectF(left, top, left + size, top + size)
                        val cx = fingerX.toInt().coerceIn(0, bitmap.width - 1); val cy = fingerY.toInt().coerceIn(0, bitmap.height - 1)
                        val radius = (size / 8).coerceAtLeast(1)
                        val source = Rect((cx - radius).coerceAtLeast(0), (cy - radius).coerceAtLeast(0), (cx + radius + 1).coerceAtMost(bitmap.width), (cy + radius + 1).coerceAtMost(bitmap.height))
                        paint.style = Paint.Style.FILL; paint.isFilterBitmap = false
                        canvas.drawBitmap(bitmap, source, target, paint)
                        paint.color = Color.CYAN; paint.style = Paint.Style.STROKE
                        canvas.drawRect(target, paint)
                        val crossX = target.left + (cx - source.left + 0.5f) / source.width() * target.width()
                        val crossY = target.top + (cy - source.top + 0.5f) / source.height() * target.height()
                        canvas.drawLine(crossX - 8, crossY, crossX + 8, crossY, paint)
                        canvas.drawLine(crossX, crossY - 8, crossX, crossY + 8, paint)
                    }
                }
                canvas.restore()
            }
        }
    }
}
