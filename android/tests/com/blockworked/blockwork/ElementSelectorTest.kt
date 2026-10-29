package com.blockworked.blockwork
import org.junit.Assert.*
import org.junit.Test

class ElementSelectorTest {
    @Test fun combinesAllFieldsAndPackageIdentity() {
        val selector = ElementSelector("example.app", "example.app:id/save", "Save", "Save document", "android.widget.Button")
        assertTrue(selector.matches("example.app", "example.app:id/save", "Save", "Save document", "android.widget.Button"))
        assertFalse(selector.matches("other.app", "example.app:id/save", "Save", "Save document", "android.widget.Button"))
        assertFalse(selector.matches("example.app", "example.app:id/save", "Discard", "Save document", "android.widget.Button"))
        assertFalse(selector.matches("example.app", "example.app:id/save", "Save", null, "android.widget.Button"))
        assertFalse(selector.matches("example.app", "wrong-id", "Save", "Save document", "android.widget.Button"))
        assertFalse(selector.matches("example.app", "example.app:id/save", "Save", "Save document", "android.widget.TextView"))
    }
    @Test fun missingOptionalNodeDataDoesNotPreventIdMatch() {
        assertTrue(ElementSelector("example.app", "id").matches("example.app", "id", null, null, null))
        assertFalse(ElementSelector("example.app", text = "Save").matches("example.app", null, "save", null, null))
    }
    @Test fun requiresUniqueMatchForActions() {
        val selector = ElementSelector("example.app", "id")
        selector.requireUnique(1)
        assertThrows(IllegalArgumentException::class.java) { selector.requireUnique(0) }
        assertThrows(IllegalArgumentException::class.java) { selector.requireUnique(2) }
    }
    @Test fun rejectsUnboundedSelectors() {
        assertThrows(IllegalArgumentException::class.java) { ElementSelector("") }
        assertThrows(IllegalArgumentException::class.java) { ElementSelector("example.app") }
    }
}
