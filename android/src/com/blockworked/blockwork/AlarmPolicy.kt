package com.blockworked.blockwork

enum class AlarmMode { EXACT, BEST_EFFORT, BLOCKED }
object AlarmPolicy {
    fun mode(exact: Boolean, access: Boolean) = if (!exact) AlarmMode.BEST_EFFORT else if (access) AlarmMode.EXACT else AlarmMode.BLOCKED
    fun validToken(token: String) = Regex("[a-f0-9]{32}").matches(token)
}
