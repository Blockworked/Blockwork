package com.blockworked.blockwork

import android.app.Instrumentation
import android.app.UiAutomation
import android.content.ComponentName
import android.view.ViewConfiguration
import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.Rect
import android.os.ParcelFileDescriptor
import android.os.SystemClock
import android.view.Display
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityWindowInfo
import androidx.test.platform.app.InstrumentationRegistry
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.*
import org.junit.Assume.assumeTrue
import org.junit.Before
import org.junit.Test
import java.util.ArrayDeque
import java.util.UUID
import java.util.concurrent.CompletableFuture
import java.util.concurrent.TimeUnit

/** Runs real Rust workflows through JNI and the actual bound accessibility service.
 * Install on a dedicated, unlocked device with empty Blockwork storage. No production test API.
 */
class FixtureDeviceTest {
    private val instrumentation: Instrumentation = InstrumentationRegistry.getInstrumentation()
    private val ui get() = instrumentation.getUiAutomation(UiAutomation.FLAG_DONT_SUPPRESS_ACCESSIBILITY_SERVICES)
    private var macroId: String? = null
    private var macroName = ""
    private val fixture = "com.blockworked.fixture"
    private val component = "com.blockworked.blockwork/com.blockworked.blockwork.AutomationService"

    @Before fun prepare() {
        // Connecting UiAutomation with this flag preserves the service under test.
        ui
        assertEquals("An unrelated workflow is running; use a dedicated device", 0, NativeRuntime.runningCount())
        val macros = state().optJSONArray("macros_data") ?: JSONArray()
        for (index in 0 until macros.length()) assertTrue("Use empty Blockwork storage on a dedicated test device",
            macros.getJSONObject(index).getString("name").startsWith("[fixture]"))
        assertTrue("Enable Blockwork automation in Accessibility settings and unlock this device",
            main { AutomationService.current?.isReady == true })
        call("new_macro")
        macroId = state().getJSONObject("current_macro").getString("id")
        macroName = "[fixture] ${UUID.randomUUID()}"
        call("set_title", obj("title" to macroName))
        call("create_variable", obj("name" to "result"))
        scene("main")
    }
    @After fun clean() {
        if (macroId == null) return
        if (NativeRuntime.runningCount() > 0) {
            call("stop_macro_named", obj("name" to macroName))
            await("Stopped workflow did not deregister") { NativeRuntime.runningCount() == 0 }
        }
        call("delete_macro", obj("id" to macroId))
    }

    @Test fun launcherIntentUsesTheFixturePackage() {
        scene("gesture")
        run(listOf(touch("Home", 0, 0), instruction("LaunchAndroidApp").put("package", text(fixture)), element("Wait", selector(id = "status", text = "ready:main"))))
        assertEquals("ready:main", node("status").text)
    }
    @Test fun selectorsAreConjunctiveAndAmbiguityIsAnError() {
        run(listOf(element("Click", selector(text = "Duplicate"))), failure = true)
        assertTrue(lastErrors().any { it.contains("ambiguous", ignoreCase = true) })
        run(listOf(element("Click", selector(id = "duplicate_target", text = "Duplicate", description = "duplicate 2"))))
        awaitText("status", "duplicate:2")
        run(listOf(element("Exists", selector(id = "duplicate_target", text = "Wrong text"))))
        assertEquals("false", result())
    }
    @Test fun replacementAndDelayedNodesAreResolvedAfresh() {
        run(listOf(element("Click", selector(id = "replace_target")), element("Exists", selector(id = "changing_target", text = "Old target"))))
        assertEquals("false", result())
        run(listOf(element("Click", selector(id = "changing_target", text = "New target"))))
        awaitText("status", "new-clicked")
        run(listOf(element("Click", selector(id = "show_delayed")), element("Wait", selector(id = "delayed_label")), element("ReadText", selector(id = "delayed_label"))))
        assertEquals("Arrived", result())
    }
    @Test fun replacementAndFocusedInsertionHaveDifferentSemantics() {
        run(listOf(element("Click", selector(id = "focus_middle")), instruction("Text").put("text", text("BLOCK"))))
        awaitText("input", "alpha BLOCK")
        run(listOf(element("SetText", selector(id = "input"), replacement = "replacement"), element("ReadText", selector(id = "input"))))
        assertEquals("replacement", result())
    }
    @Test fun passwordTextCannotBeReadOrInserted() {
        scene("password")
        run(listOf(instruction("Text").put("text", text("must-not-insert"))), failure = true)
        assertTrue(lastErrors().any { it.contains("editable text", ignoreCase = true) })
        run(listOf(element("ReadText", selector(id = "password"))), failure = true)
        assertTrue(lastErrors().any { it.contains("readable text", ignoreCase = true) })
    }
    @Test fun physicalPixelGesturesAndConcurrentStrandsCompleteInOrder() {
        scene("gesture")
        val box = node("gesture_area").bounds
        val x = box.centerX(); val y = box.centerY()
        run(listOf(touch("Tap", x, y), touch("LongPress", x, y, duration = ViewConfiguration.getLongPressTimeout() + 300), touch("Swipe", box.left + box.width()/4, y, box.right - box.width()/4, y, 400)))
        awaitText("status", "taps:1 holds:1 swipes:1")
        scene("gesture")
        val live = node("gesture_area").bounds
        val pending = startRuns(listOf(listOf(touch("Tap", live.centerX(), live.centerY())), listOf(touch("Tap", live.centerX(), live.centerY()))))
        finish(pending)
        awaitText("status", "taps:2 holds:0 swipes:0")
    }
    @Test fun scrollActionsMoveTheRealViewport() {
        scene("scroll")
        run(listOf(element("ScrollForward", selector(id = "scroll_panel"))))
        await("ScrollForward did not change the viewport") { (node("status").text?.removePrefix("scroll:")?.toIntOrNull() ?: 0) > 0 }
        val moved = node("status").text!!.removePrefix("scroll:").toInt()
        run(listOf(element("ScrollBackward", selector(id = "scroll_panel"))))
        await("ScrollBackward did not change the viewport") { (node("status").text?.removePrefix("scroll:")?.toIntOrNull() ?: moved) < moved }
    }
    @Test fun waitsAreCatchableAndStopDoesNotReplayPendingActions() {
        val absent = selector(id = "never-present")
        val catcher = instruction("TryCatch").put("body", JSONArray().put(element("Wait", absent, timeout = 200)))
            .put("catch_body", JSONArray().put(instruction("SetVariable").put("name", "result").put("value", text("caught"))))
        run(listOf(catcher)); assertEquals("caught", result())
        val pending = startRuns(listOf(listOf(element("Wait", absent, timeout = 0), element("Click", selector(id = "click_target")))))
        await("Infinite wait did not start") { answerPermissions(); NativeRuntime.runningCount() > 0 }
        call("stop_macro_named", obj("name" to macroName))
        await("Stop did not end the pending wait") { NativeRuntime.runningCount() == 0 && endedSince(pending.seq) }
        assertFalse(logSince(pending.seq).any { it.optString("message") in pending.markers })
        assertEquals("ready:main", node("status").text)
    }
    @Test fun screenshotPixelsUsePhysicalCoordinatesAndCoalescedCopies() {
        scene("gesture")
        val box = node("gesture_area").bounds
        val first = CompletableFuture<Result<Bitmap>>(); val second = CompletableFuture<Result<Bitmap>>()
        main {
            val service = AutomationService.current ?: error("Service disconnected")
            service.screenshots.request { first.complete(it) }
            service.screenshots.request { second.complete(it) }
        }
        val firstResult = first.get(10, TimeUnit.SECONDS)
        val secondResult = second.get(10, TimeUnit.SECONDS)
        try {
            val a = firstResult.getOrThrow()
            val b = secondResult.getOrThrow()
            assertNotSame(a, b); assertEquals(a.width, b.width); assertEquals(a.height, b.height)
            val expected = intArrayOf(Color.RED, Color.GREEN, Color.BLUE, Color.YELLOW)
            for (index in expected.indices) {
                val x = box.left + box.width() * (if (index % 2 == 0) 1 else 3) / 4
                val y = box.top + box.height() * (if (index < 2) 1 else 3) / 4
                assertEquals("Fixture pixel at physical coordinate $x,$y", expected[index], a.getPixel(x, y))
                assertEquals(a.getPixel(x, y), b.getPixel(x, y))
            }
        } finally { firstResult.getOrNull()?.recycle(); secondResult.getOrNull()?.recycle() }
    }
    @Test fun secureWindowMustReturnACaptureError() {
        scene("secure")
        val capture = CompletableFuture<Result<Bitmap>>()
        main { AutomationService.current!!.screenshots.request { capture.complete(it) } }
        val answer = capture.get(10, TimeUnit.SECONDS)
        // A masked black bitmap is NOT a pass. This intentionally exposes the
        // unresolved secure-layer gap on Android versions that silently mask it.
        answer.getOrNull()?.recycle()
        assertTrue("Secure content returned a bitmap instead of an actionable error; release gate remains unmet", answer.isFailure)
        assertTrue("Capture error did not identify protected content: ${answer.exceptionOrNull()}", answer.exceptionOrNull()?.message?.contains("secure", ignoreCase = true) == true)
    }
    @Test fun serviceRevocationStopsTheRunAndReconnectDoesNotReplay() {
        assumeTrue("Opt in with -e includeRevocation true on a dedicated device",
            InstrumentationRegistry.getArguments().getString("includeRevocation") == "true")
        val original = shell("settings get secure enabled_accessibility_services").trim()
        val services = original.split(':').filter { it.isNotBlank() && it != "null" }
        fun isBlockwork(value: String) = ComponentName.unflattenFromString(value) == ComponentName.unflattenFromString(component)
        assertTrue("Expected the Blockwork service to be enabled", services.any(::isBlockwork))
        val pending = startRuns(listOf(listOf(element("Wait", selector(id = "never-present"), timeout = 0), element("Click", selector(id = "click_target")))))
        await("Run did not start") { answerPermissions(); NativeRuntime.runningCount() > 0 }
        try {
            shell("settings put secure enabled_accessibility_services ${quote(services.filterNot(::isBlockwork).joinToString(":"))}")
            await("Service revocation did not stop the workflow", 20000) { NativeRuntime.runningCount() == 0 && !main { AutomationService.current?.isReady == true } }
        } finally { shell("settings put secure enabled_accessibility_services ${quote(original)}") }
        await("Accessibility did not reconnect", 20000) { main { AutomationService.current?.isReady == true } }
        assertFalse(logSince(pending.seq).any { it.optString("message") in pending.markers })
        assertEquals("ready:main", node("status").text)
    }

    private data class Pending(val seq: Long, val markers: List<String>)
    private fun startRuns(bodies: List<List<JSONObject>>): Pending {
        call("clear_instructions")
        val markers = bodies.map { "fixture-done-${UUID.randomUUID()}" }
        for ((branch, body) in bodies.withIndex()) {
            val strand = call("add_strand", obj("x" to 20, "y" to branch*100, "instruction" to instruction("WhenRan")))
            for ((index, action) in (body + instruction("Log").put("value", text(markers[branch]))).withIndex())
                call("add_instruction", obj("strandId" to strand, "path" to JSONArray().put(obj("index" to index+1)), "instruction" to action))
        }
        val seq = logs().maxOfOrNull { it.optLong("seq") } ?: -1
        call("run_macro")
        return Pending(seq, markers)
    }
    private fun run(body: List<JSONObject>, failure: Boolean = false) = finish(startRuns(listOf(body)), failure)
    private fun finish(pending: Pending, failure: Boolean = false) {
        await("Workflow did not end: ${logSince(pending.seq)}", 20000) { answerPermissions(); endedSince(pending.seq) && NativeRuntime.runningCount() == 0 }
        val entries = logSince(pending.seq)
        val errors = entries.filter { it.optString("level") == "Error" }
        if (failure) { assertTrue("Expected workflow error: $entries", errors.isNotEmpty()); assertFalse(entries.any { it.optString("message") in pending.markers }) }
        else { assertEquals("Workflow failed: $errors", 0, errors.size); for (marker in pending.markers) assertTrue("No completion marker: $entries", entries.any { it.optString("message") == marker }) }
    }
    private fun endedSince(seq: Long) = logSince(seq).any { it.optString("event") == "Ended" }
    private fun logs(): List<JSONObject> { val rows = state().optJSONArray("run_log") ?: JSONArray(); return (0 until rows.length()).map { rows.getJSONObject(it) }.filter { it.optString("macro_id") == macroId } }
    private fun logSince(seq: Long) = logs().filter { it.optLong("seq") > seq }
    private fun lastErrors() = logs().filter { it.optString("level") == "Error" }.map { it.optString("message") }
    private fun result() = call("preview_value", obj("value" to obj("kind" to "Var", "name" to "result"))).toString()
    private fun answerPermissions() {
        val prompt = NativeRuntime.checked(NativeRuntime.pendingPrompt()) as? JSONObject ?: return
        assertEquals("Unexpected workflow prompt", "permission", prompt.getJSONObject("request").getString("kind"))
        NativeRuntime.checked(NativeRuntime.answerPrompt(prompt.getLong("id"), obj("answer" to "once").toString()))
    }
    private fun scene(name: String) {
        require(name in listOf("main", "gesture", "scroll", "password", "secure"))
        val output = shell("am start -W -n $fixture/.MainActivity --es scene $name")
        assertFalse("Fixture launch failed: $output", output.contains("Error:"))
        await("Fixture scene $name not ready") { runCatching { node("status").text == "ready:$name" }.getOrDefault(false) }
        // Wait for the target's first compositor frame before coordinate checks.
        instrumentation.waitForIdleSync()
        SystemClock.sleep(150)
    }
    private data class Node(val text: String?, val bounds: Rect)
    @Suppress("DEPRECATION")
    private fun node(id: String): Node = main {
        val service = AutomationService.current ?: error("Accessibility service disconnected")
        val queue = ArrayDeque<AccessibilityNodeInfo>()
        try {
            for (window in service.windows) try {
                if (window.type == AccessibilityWindowInfo.TYPE_APPLICATION && window.displayId == Display.DEFAULT_DISPLAY)
                    window.root?.let { if (it.packageName?.toString() == fixture) queue.add(it) else it.recycle() }
            } finally { window.recycle() }
            var visited = 0
            while (queue.isNotEmpty()) {
                val item = queue.removeFirst()
                try {
                    check(++visited <= 10000) { "Fixture tree exceeded bound" }
                    if (item.viewIdResourceName == "$fixture:id/$id") return@main Node(item.text?.toString(), Rect().also { item.getBoundsInScreen(it) })
                    for (index in 0 until item.childCount) item.getChild(index)?.let { queue.add(it) }
                } finally { item.recycle() }
            }
            error("Fixture node $id not found")
        } finally { queue.forEach { it.recycle() } }
    }
    private fun awaitText(id: String, value: String) = await("$id did not become '$value'") { node(id).text == value }
    private fun <T> main(action: () -> T): T { val reply = CompletableFuture<T>(); instrumentation.runOnMainSync { try { reply.complete(action()) } catch (error: Throwable) { reply.completeExceptionally(error) } }; return reply.get(10, TimeUnit.SECONDS) }
    private fun await(message: String, timeout: Long = 10000, test: () -> Boolean) {
        val end = SystemClock.uptimeMillis() + timeout
        var lastError: Exception? = null
        while (true) {
            try { if (test()) return } catch (error: Exception) { lastError = error }
            if (SystemClock.uptimeMillis() >= end) fail("$message${lastError?.let { ": $it" } ?: ""}")
            SystemClock.sleep(25)
        }
    }
    private fun shell(command: String): String = ParcelFileDescriptor.AutoCloseInputStream(ui.executeShellCommand(command)).use { it.bufferedReader().readText() }
    private fun quote(value: String) = "'" + value.replace("'", "'\\''") + "'"
    private fun state() = call("get_state") as JSONObject
    private fun call(command: String, args: JSONObject = JSONObject()): Any = NativeRuntime.checked(NativeRuntime.call(command, args.toString())) ?: JSONObject.NULL
    private fun obj(vararg fields: Pair<String, Any?>) = JSONObject().apply { fields.forEach { put(it.first, it.second ?: JSONObject.NULL) } }
    private fun text(value: String) = obj("kind" to "Text", "value" to value)
    private fun number(value: Int) = obj("kind" to "Number", "value" to value)
    private fun instruction(type: String) = obj("id" to UUID.randomUUID().toString(), "type" to type)
    private fun selector(id: String = "", text: String = "", description: String = "") = obj("package" to fixture, "resource_id" to if (id.isEmpty()) "" else "$fixture:id/$id", "text" to text, "description" to description, "class" to "")
    private fun element(action: String, selector: JSONObject, replacement: String = "", timeout: Int = 2000) = instruction("Element").put("selector", selector).put("action", action).put("text", text(replacement)).put("timeout", number(timeout)).put("variable", "result")
    private fun touch(action: String, x: Int, y: Int, endX: Int = x, endY: Int = y, duration: Int = 80) = instruction("Touch").put("action", action).put("x", number(x)).put("y", number(y)).put("end_x", number(endX)).put("end_y", number(endY)).put("duration", number(duration))
}
