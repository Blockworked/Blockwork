package com.blockworked.fixture

import android.app.Activity
import android.content.Intent
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.InputType
import android.view.MotionEvent
import android.view.View
import android.view.ViewConfiguration
import android.view.WindowInsets
import android.view.WindowManager
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/** Standalone, permission-free targets for Blockwork device validation. */
class MainActivity : Activity() {
    private val handler = Handler(Looper.getMainLooper())
    private lateinit var status: TextView
    private fun dp(value: Int) = (value * resources.displayMetrics.density).toInt()
    override fun onCreate(state: Bundle?) { super.onCreate(state); showScene() }
    override fun onNewIntent(intent: Intent) { super.onNewIntent(intent); setIntent(intent); showScene() }
    override fun onDestroy() { handler.removeCallbacksAndMessages(null); super.onDestroy() }
    private fun showScene() {
        handler.removeCallbacksAndMessages(null)
        val scene = intent.getStringExtra("scene") ?: "main"
        if (scene == "secure") window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
        else window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
        val root = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setBackgroundColor(Color.WHITE) }
        status = label(root, R.id.status, "ready:$scene")
        val column = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        val scroll = ScrollView(this).apply { id = R.id.scroll_panel; addView(column); isFillViewport = true }
        root.addView(scroll, LinearLayout.LayoutParams(-1, 0, 1f))
        root.setOnApplyWindowInsetsListener { view, insets ->
            val bars = insets.getInsets(WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout())
            view.setPadding(bars.left, bars.top, bars.right, bars.bottom); insets
        }
        setContentView(root)
        when (scene) {
            "gesture", "secure" -> {
                label(column, View.NO_ID, if (scene == "secure") "Protected screen: capture must fail" else "Physical-pixel gesture and capture target")
                column.addView(GesturePad(), LinearLayout.LayoutParams(-1, dp(240)))
            }
            "scroll" -> {
                scroll.setOnScrollChangeListener { _, _, y, _, oldY -> if (y != oldY) status.text = "scroll:$y" }
                for (index in 1..35) label(column, View.NO_ID, "Scroll row $index").minHeight = dp(56)
                label(column, R.id.scroll_end, "Scroll end")
            }
            "password" -> {
                edit(column, R.id.password, "fixture-secret", true).apply { requestFocus(); setSelection(text.length) }
            }
            else -> {
                button(column, R.id.click_target, "Tap target") { status.text = "clicked" }
                button(column, R.id.long_target, "Long press target") {}.setOnLongClickListener { status.text = "long-clicked"; true }
                val input = edit(column, R.id.input, "alpha beta")
                button(column, R.id.focus_middle, "Select beta") { input.requestFocus(); input.setSelection(6, 10) }
                repeat(2) { index ->
                    button(column, R.id.duplicate_target, "Duplicate") { status.text = "duplicate:${index + 1}" }
                        .contentDescription = "duplicate ${index + 1}"
                }
                val changing = LinearLayout(this).also { it.orientation = LinearLayout.VERTICAL; column.addView(it) }
                button(changing, R.id.changing_target, "Old target") { status.text = "old-clicked" }
                button(column, R.id.replace_target, "Replace target") {
                    changing.removeAllViews()
                    button(changing, R.id.changing_target, "New target") { status.text = "new-clicked" }
                }
                button(column, R.id.show_delayed, "Show delayed element") {
                    handler.postDelayed({
                        if (column.findViewById<View>(R.id.delayed_label) == null) label(column, R.id.delayed_label, "Arrived")
                    }, 300)
                }
                for (target in listOf("gesture", "scroll", "password", "secure"))
                    button(column, View.NO_ID, "Open $target scene") { startActivity(Intent(this, MainActivity::class.java).putExtra("scene", target)) }
            }
        }
    }
    private fun label(parent: LinearLayout, viewId: Int, value: String): TextView = TextView(this).apply {
        id = viewId; text = value; textSize = 18f; setTextColor(Color.BLACK); setPadding(dp(12), dp(8), dp(12), dp(8))
        parent.addView(this, LinearLayout.LayoutParams(-1, -2))
    }
    private fun button(parent: LinearLayout, viewId: Int, value: String, clicked: () -> Unit): Button = Button(this).apply {
        id = viewId; text = value; isAllCaps = false; minHeight = dp(48)
        setOnClickListener { clicked() }; parent.addView(this, LinearLayout.LayoutParams(-1, -2))
    }
    private fun edit(parent: LinearLayout, viewId: Int, value: String, password: Boolean = false): EditText = EditText(this).apply {
        id = viewId; inputType = InputType.TYPE_CLASS_TEXT or if (password) InputType.TYPE_TEXT_VARIATION_PASSWORD else InputType.TYPE_TEXT_FLAG_MULTI_LINE
        setText(value); minHeight = dp(48); parent.addView(this, LinearLayout.LayoutParams(-1, -2))
    }
    private inner class GesturePad : View(this@MainActivity) {
        private val paint = Paint()
        private var downX = 0f
        private var downY = 0f
        private var held = false
        private var taps = 0
        private var holds = 0
        private var swipes = 0
        private val hold = Runnable { held = true; performLongClick() }
        init {
            id = R.id.gesture_area; isClickable = true; isLongClickable = true
            contentDescription = "Four-color gesture pad"
        }
        override fun onDraw(canvas: Canvas) {
            val colors = intArrayOf(Color.RED, Color.GREEN, Color.BLUE, Color.YELLOW)
            for (index in colors.indices) {
                paint.color = colors[index]
                val x = if (index % 2 == 0) 0f else width / 2f
                val y = if (index < 2) 0f else height / 2f
                canvas.drawRect(x, y, x + width / 2f, y + height / 2f, paint)
            }
        }
        override fun performClick(): Boolean { super.performClick(); taps++; report(); return true }
        override fun performLongClick(): Boolean { holds++; report(); return true }
        private fun report() { status.text = "taps:$taps holds:$holds swipes:$swipes" }
        override fun onTouchEvent(event: MotionEvent): Boolean {
            when (event.actionMasked) {
                MotionEvent.ACTION_DOWN -> { downX = event.x; downY = event.y; held = false; handler.postDelayed(hold, ViewConfiguration.getLongPressTimeout().toLong()); parent.requestDisallowInterceptTouchEvent(true) }
                MotionEvent.ACTION_MOVE -> if (kotlin.math.hypot(event.x - downX, event.y - downY) > dp(24)) handler.removeCallbacks(hold)
                MotionEvent.ACTION_UP -> {
                    handler.removeCallbacks(hold)
                    if (kotlin.math.hypot(event.x - downX, event.y - downY) > dp(40)) { swipes++; report() }
                    else if (!held) performClick()
                    parent.requestDisallowInterceptTouchEvent(false)
                }
                MotionEvent.ACTION_CANCEL -> { handler.removeCallbacks(hold); parent.requestDisallowInterceptTouchEvent(false) }
            }
            return true
        }
    }
}
