package com.blockworked.blockwork

import android.Manifest
import android.app.AlertDialog
import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import android.widget.EditText
import android.widget.Toast
import org.json.JSONObject
import org.qtproject.qt.android.QtQuickView
import java.io.File
import java.lang.ref.WeakReference
import java.util.UUID

class BlockworkActivity : Activity() {
    private val host get() = application as BlockworkApplication
    private var promptId = 0L
    private var dialog: AlertDialog? = null
    private var exportId = ""
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(QtQuickView(this, "qrc:/qt/qml/com/blockworked/Android/qml/Main.qml", "blockwork-android"))
        exportId = savedInstanceState?.getString("exportId") ?: ""
        if (Build.VERSION.SDK_INT >= 33 && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != android.content.pm.PackageManager.PERMISSION_GRANTED)
            requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 10)
        acceptShare(intent)
    }
    override fun onSaveInstanceState(state: Bundle) {
        state.putString("exportId", exportId)
        super.onSaveInstanceState(state)
    }
    override fun onResume() {
        super.onResume()
        current = WeakReference(this)
        host.activity = WeakReference(this)
    }
    @Deprecated("Legacy Back callback is used with enableOnBackInvokedCallback=false")
    override fun onBackPressed() {
        // QtQuickView is embedded in a plain Activity, so Activity Back must be
        // forwarded explicitly. The Qt thread dismisses IME/popups first.
        pendingBack.set(true)
    }
    override fun onPause() {
        host.activity.clear()
        current.clear()
        pendingBack.set(false)
        dialog?.setOnCancelListener(null)
        dialog?.dismiss()
        dialog = null
        promptId = 0
        super.onPause()
    }
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        acceptShare(intent)
    }
    @Suppress("DEPRECATION")
    private fun acceptShare(intent: Intent?) {
        if (intent?.action == Intent.ACTION_SEND) {
            val uri = intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)
            // Consume once: rotation/recreation must not re-import the same share.
            intent.removeExtra(Intent.EXTRA_STREAM)
            if (uri != null) importUri(uri)
        }
    }
    private fun importUri(uri: Uri) = host.worker.execute {
        val copy = File(filesDir, "imports/${UUID.randomUUID()}.macro")
        try {
            copy.parentFile?.mkdirs()
            contentResolver.openInputStream(uri).use { input ->
                requireNotNull(input) { "The document could not be opened" }
                copy.outputStream().use { output ->
                    val buffer = ByteArray(8192)
                    var total = 0L
                    while (true) {
                        val count = input.read(buffer)
                        if (count < 0) break
                        total += count
                        require(total <= 32 * 1024 * 1024) { "Workflow document exceeds 32 MB" }
                        output.write(buffer, 0, count)
                    }
                }
            }
            NativeRuntime.checked(NativeRuntime.call("import_macro", JSONObject().put("path", copy.absolutePath).toString()))
        } catch (error: Exception) { report(error) }
        finally { copy.delete() }
    }
    @Deprecated("Android activity result callback")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (resultCode != RESULT_OK) return
        val uri = data?.data ?: return
        if (requestCode == 1) importUri(uri)
        if (requestCode == 2) {
            val id = exportId
            host.worker.execute {
                val copy = File(cacheDir, "${UUID.randomUUID()}.macro")
                try {
                    NativeRuntime.checked(NativeRuntime.call("export_macro", JSONObject().put("macroId", id).put("path", copy.absolutePath).toString()))
                    contentResolver.openOutputStream(uri, "wt").use { output ->
                        requireNotNull(output) { "The destination could not be opened" }
                        copy.inputStream().use { it.copyTo(output) }
                    }
                } catch (error: Exception) { report(error) }
                finally { copy.delete() }
            }
        }
    }
    private fun report(error: Exception) = runOnUiThread {
        Toast.makeText(this, error.message ?: "Workflow operation failed", Toast.LENGTH_LONG).show()
    }
    fun showPrompt(pending: JSONObject?) {
        val id = pending?.optLong("id") ?: 0L
        if (id == promptId) return
        dialog?.setOnCancelListener(null)
        dialog?.dismiss()
        dialog = null
        promptId = id
        if (pending == null) return
        val request = pending.getJSONObject("request")
        fun answer(button: String, text: String = "") {
            NativeRuntime.answerPrompt(id, JSONObject().put("answer", button).put("text", text).toString())
        }
        val builder = AlertDialog.Builder(this).setTitle(request.optString("title"))
            .setMessage(request.optString("message")).setNegativeButton("Cancel") { _, _ -> answer("cancel") }
            .setOnCancelListener { answer("cancel") }
        when (request.optString("kind")) {
            "text" -> {
                val input = EditText(this).apply { setText(request.optString("default")); minHeight = 48 }
                builder.setView(input).setPositiveButton("OK") { _, _ -> answer("ok", input.text.toString()) }
            }
            "confirm" -> builder.setPositiveButton("Yes") { _, _ -> answer("yes") }
                .setNeutralButton("No") { _, _ -> answer("no") }
            "permission" -> builder.setPositiveButton("Allow once") { _, _ -> answer("once") }
                .setNeutralButton("Always allow") { _, _ -> answer("always") }
                .setNegativeButton("Deny") { _, _ -> answer("deny") }
            "choice" -> {
                val options = request.getJSONArray("options")
                // A choice reply's text is the option, matching the shared prompt contract.
                builder.setMessage(null).setItems(Array(options.length()) { options.getString(it) }) { _, index -> answer("ok", options.getString(index)) }
            }
            else -> builder.setPositiveButton("OK") { _, _ -> answer("ok") }
        }
        dialog = builder.show()
    }

    companion object {
        private var current = WeakReference<BlockworkActivity>(null)
        private val pendingBack = java.util.concurrent.atomic.AtomicBoolean(false)
        @JvmStatic fun takeBackPress(): Boolean = pendingBack.getAndSet(false)
        @JvmStatic fun backgroundEditor() { current.get()?.let { activity -> activity.runOnUiThread { activity.moveTaskToBack(true) } } }
        @JvmStatic fun importDocument() { current.get()?.let { activity -> activity.runOnUiThread {
            activity.startActivityForResult(Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                addCategory(Intent.CATEGORY_OPENABLE); type = "*/*"
            }, 1)
        } } }
        @JvmStatic fun exportDocument(macroId: String) { current.get()?.let { activity -> activity.runOnUiThread {
            activity.exportId = macroId
            activity.startActivityForResult(Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
                addCategory(Intent.CATEGORY_OPENABLE); type = "application/json"; putExtra(Intent.EXTRA_TITLE, "workflow.macro")
            }, 2)
        } } }
        @JvmStatic fun exactAlarmSettings() { current.get()?.let { activity -> activity.runOnUiThread {
            if (Build.VERSION.SDK_INT >= 31) activity.startActivity(Intent(Settings.ACTION_REQUEST_SCHEDULE_EXACT_ALARM, Uri.parse("package:" + activity.packageName)))
            else Toast.makeText(activity, "Android 11 allows exact alarms without separate access", Toast.LENGTH_SHORT).show()
        } } }
        @JvmStatic fun accessibilitySettings() { current.get()?.let { activity -> activity.runOnUiThread {
            activity.startActivity(Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS))
        } } }
    }
}
