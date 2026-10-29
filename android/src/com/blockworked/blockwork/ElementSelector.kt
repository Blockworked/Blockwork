package com.blockworked.blockwork

/** Exact, conjunctive matching; empty optional fields are unconstrained. */
data class ElementSelector(val app: String, val resourceId: String = "", val text: String = "", val description: String = "", val className: String = "") {
    init {
        require(app.isNotBlank()) { "Choose the target app package" }
        require(listOf(resourceId, text, description, className).any { it.isNotEmpty() }) { "Supply an element resource ID, text, description or class" }
    }
    fun requireUnique(count: Int) {
        require(count > 0) { "Element is not present; check the selector or wait for it" }
        require(count == 1) { "Element selector is ambiguous; add another matching field" }
    }
    fun matches(app: String?, resourceId: String?, text: String?, description: String?, className: String?): Boolean =
        this.app == app && (this.resourceId.isEmpty() || this.resourceId == resourceId) &&
        (this.text.isEmpty() || this.text == text) && (this.description.isEmpty() || this.description == description) &&
        (this.className.isEmpty() || this.className == className)
}
