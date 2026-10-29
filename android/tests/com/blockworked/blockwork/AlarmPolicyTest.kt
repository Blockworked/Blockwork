package com.blockworked.blockwork
import org.junit.Assert.*
import org.junit.Test
class AlarmPolicyTest {
    @Test fun exactAccessIsNeverSilentlyDowngraded() {
        assertEquals(AlarmMode.BLOCKED, AlarmPolicy.mode(true, false))
        assertEquals(AlarmMode.EXACT, AlarmPolicy.mode(true, true))
        assertEquals(AlarmMode.BEST_EFFORT, AlarmPolicy.mode(false, false))
        assertEquals(AlarmMode.BEST_EFFORT, AlarmPolicy.mode(false, true))
    }
    @Test fun broadcastIdentitiesAreOpaqueTokensNotWorkflowCommands() {
        assertTrue(AlarmPolicy.validToken("0123456789abcdef0123456789abcdef"))
        for (value in listOf("", "workflow-id", "../../file", "intent://app", "echo bad", "0123456789abcdef0123456789abcdeF")) assertFalse(AlarmPolicy.validToken(value))
    }
}
