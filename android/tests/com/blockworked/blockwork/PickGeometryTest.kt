package com.blockworked.blockwork

import org.junit.Assert.*
import org.junit.Test

class PickGeometryTest {
    @Test fun pointsArePhysicalPixelsIncludingDisplayEdges() {
        assertEquals(PickRect(123, 456, 0, 0), PickRect.point(123.9f, 456.5f, 1080, 2400))
        assertEquals(PickRect(1079, 2399, 0, 0), PickRect.point(1080f, 2400f, 1080, 2400))
        assertEquals(PickRect(0, 0, 0, 0), PickRect.point(-3f, -2f, 1080, 2400))
    }
    @Test fun reverseDragCoversAllSelectedPixels() {
        assertEquals(PickRect(10, 20, 91, 181), PickRect.region(100.1f, 200.4f, 10.2f, 20.8f, 1080, 2400))
        assertEquals(PickRect(0, 0, 1080, 2400), PickRect.region(-10f, -20f, 1100f, 3000f, 1080, 2400))
    }
    @Test fun landscapeUsesItsOwnDimensionsAndKeepsRegionNonempty() {
        assertEquals(PickRect(2399, 1079, 1, 1), PickRect.region(2400f, 1080f, 2400f, 1080f, 2400, 1080))
        assertEquals(PickRect(700, 500, 0, 0), PickRect.point(700f, 500f, 2400, 1080))
    }
    @Test(expected = IllegalArgumentException::class)
    fun unavailableDisplayDoesNotProduceCoordinates() { PickRect.point(0f, 0f, 0, 0) }
}
