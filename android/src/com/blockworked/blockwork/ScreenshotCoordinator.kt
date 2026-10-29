package com.blockworked.blockwork

import android.accessibilityservice.AccessibilityService
import android.graphics.Bitmap
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.view.Choreographer
import android.view.Display

/** Main-looper owned. Coalesces callers, spaces captures and excludes our overlays. */
class ScreenshotCoordinator(private val service: AutomationService) {
    private val handler = Handler(Looper.getMainLooper())
    private val waiting = mutableListOf<(Result<Bitmap>) -> Unit>()
    private var lastAttempt = -500L
    private var busy = false
    private var closed = false
    private var generation = 0

    fun request(done: (Result<Bitmap>) -> Unit) {
        if (closed) { done(Result.failure(IllegalStateException("Accessibility service disconnected"))); return }
        waiting.add(done)
        if (busy) return
        busy = true
        val token = ++generation
        handler.postDelayed({
            if (token == generation && busy) invalidate("Android screenshot callback timed out; try again")
        }, 6000)
        handler.postDelayed({ begin(token, 0) }, (500 - (SystemClock.uptimeMillis() - lastAttempt)).coerceAtLeast(0))
    }
    fun invalidate(reason: String, close: Boolean = false) {
        generation++
        closed = close
        busy = false
        service.hideOverlaysForCapture(false)
        val callbacks = waiting.toList(); waiting.clear()
        callbacks.forEach { it(Result.failure(IllegalStateException(reason))) }
    }
    private fun begin(token: Int, retry: Int) {
        if (token != generation || closed) return
        if (!service.isReady) { finish(token, Result.failure(IllegalStateException("Unlock the device and enable accessibility"))); return }
        service.hideOverlaysForCapture(true)
        // Wait for both the layout and compositor frame that removes overlays.
        Choreographer.getInstance().postFrameCallback {
            Choreographer.getInstance().postFrameCallback captureFrame@{
                if (token != generation || closed) return@captureFrame
                lastAttempt = SystemClock.uptimeMillis()
                try {
                    val geometry = service.displayGeometry()
                    service.takeScreenshot(Display.DEFAULT_DISPLAY, service.mainExecutor,
                        object : AccessibilityService.TakeScreenshotCallback {
                            override fun onSuccess(result: AccessibilityService.ScreenshotResult) {
                                val buffer = result.hardwareBuffer
                                val bitmap = try {
                                    val hardware = Bitmap.wrapHardwareBuffer(buffer, result.colorSpace)
                                        ?: error("Android returned an unreadable screenshot")
                                    try { hardware.copy(Bitmap.Config.ARGB_8888, false) ?: error("Screenshot allocation failed") }
                                    finally { hardware.recycle() }
                                } catch (error: Exception) {
                                    finish(token, Result.failure(error)); return
                                } finally { buffer.close() }
                                if (token != generation || geometry != runCatching { service.displayGeometry() }.getOrNull() || !service.isReady) {
                                    bitmap.recycle()
                                    finish(token, Result.failure(IllegalStateException("Display or accessibility access changed during capture")))
                                } else finish(token, Result.success(bitmap))
                            }
                            override fun onFailure(code: Int) {
                                if (token != generation) return
                                // Android may rate-limit captures taken by another client too.
                                if (code == AccessibilityService.ERROR_TAKE_SCREENSHOT_INTERVAL_TIME_SHORT && retry == 0) {
                                    service.hideOverlaysForCapture(false)
                                    handler.postDelayed({ begin(token, 1) }, 750)
                                    return
                                }
                                val message = when (code) {
                                    AccessibilityService.ERROR_TAKE_SCREENSHOT_INTERVAL_TIME_SHORT -> "Android screenshot rate limit; try again shortly"
                                    AccessibilityService.ERROR_TAKE_SCREENSHOT_NO_ACCESSIBILITY_ACCESS -> "Screenshot access was revoked; re-enable accessibility"
                                    AccessibilityService.ERROR_TAKE_SCREENSHOT_INVALID_DISPLAY -> "The default display is unavailable"
                                    6 -> "Android blocked capture of secure content"
                                    else -> "Android could not capture the screen (error $code); secure content cannot be captured"
                                }
                                finish(token, Result.failure(IllegalStateException(message)))
                            }
                        })
                } catch (error: Exception) { finish(token, Result.failure(error)) }
            }
        }
    }
    private fun finish(token: Int, result: Result<Bitmap>) {
        if (token != generation) { result.getOrNull()?.recycle(); return }
        service.hideOverlaysForCapture(false)
        busy = false
        val callbacks = waiting.toList(); waiting.clear()
        // Each caller owns its software copy. No view or workflow retains a
        // hardware buffer or a cached frame after service/geometry changes.
        val source = result.getOrNull()
        try {
            callbacks.forEach { callback ->
                val delivery = if (source == null) result else runCatching {
                    source.copy(Bitmap.Config.ARGB_8888, false) ?: error("Screenshot allocation failed")
                }
                callback(delivery)
            }
        } finally { source?.recycle() }
    }
}
