package com.blockworked.blockwork

import org.json.JSONObject

object NativeRuntime {
    init { System.loadLibrary("blockwork_android") }
    @JvmStatic external fun start(root: String): String
    @JvmStatic external fun call(command: String, arguments: String): String
    @JvmStatic external fun pendingPrompt(): String
    @JvmStatic external fun answerPrompt(id: Long, reply: String): String
    @JvmStatic external fun alarmPlan(nowMs: Long, zone: String, exactAccess: Boolean): String
    @JvmStatic external fun deliverAlarm(token: String, nowMs: Long, zone: String, exactAccess: Boolean): String
    @JvmStatic external fun scheduleUiReady(ready: Boolean)
    @JvmStatic external fun foregroundApp(packageName: String): String
    @JvmStatic external fun serviceReady(ready: Boolean): String
    @JvmStatic external fun takeActions(): String
    @JvmStatic external fun actionReply(id: Long, reply: String): String
    @JvmStatic external fun displayChanged(width: Int, height: Int): String
    @JvmStatic external fun screenshotReply(id: Long, width: Int, height: Int, argb: IntArray): String
    @JvmStatic external fun runningCount(): Int

    fun checked(raw: String): Any? {
        val answer = JSONObject(raw)
        if (!answer.optBoolean("ok")) throw IllegalStateException(answer.optString("error"))
        return answer.opt("result")
    }
}
