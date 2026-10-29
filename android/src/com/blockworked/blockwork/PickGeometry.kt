package com.blockworked.blockwork

import kotlin.math.floor
import kotlin.math.max
import kotlin.math.min

/** Physical pixels, independent of Qt's logical coordinate system. */
data class PickRect(val x: Int, val y: Int, val width: Int, val height: Int) {
    companion object {
        fun point(x: Float, y: Float, displayWidth: Int, displayHeight: Int): PickRect {
            require(displayWidth > 0 && displayHeight > 0) { "Display is unavailable" }
            return PickRect(floor(x.toDouble()).toInt().coerceIn(0, displayWidth - 1),
                floor(y.toDouble()).toInt().coerceIn(0, displayHeight - 1), 0, 0)
        }
        fun region(x0: Float, y0: Float, x1: Float, y1: Float, displayWidth: Int, displayHeight: Int): PickRect {
            require(displayWidth > 0 && displayHeight > 0) { "Display is unavailable" }
            val left = floor(min(x0, x1).toDouble()).toInt().coerceIn(0, displayWidth - 1)
            val top = floor(min(y0, y1).toDouble()).toInt().coerceIn(0, displayHeight - 1)
            val right = kotlin.math.ceil(max(x0, x1).toDouble()).toInt().coerceIn(left + 1, displayWidth)
            val bottom = kotlin.math.ceil(max(y0, y1).toDouble()).toInt().coerceIn(top + 1, displayHeight)
            return PickRect(left, top, right - left, bottom - top)
        }
    }
}
