import QtQml

// Stands in for AppBridge (blockwork-qt/src/app_bridge.rs) in tests. Each
// callCommand is logged in `calls` and answered at once, unless `silent`.
QtObject {
    property int replySerial: 0
    property string replyKey: ""
    property string replyJson: "null"
    property string replyError: ""
    property string lastError: ""
    property int previewSerial: 0
    property string activeBlocksJson: "[]"
    property string installedAppsError: ""

    // Every callCommand so far, as {command, args}.
    property var calls: []
    // The error replies carry; "" answers with success.
    property string failure: ""
    // A reply's value by command name, else `answer`.
    property var answers: ({})
    property var answer: null
    // Log calls but never reply.
    property bool silent: false

    function callCommand(key, command, args) {
        calls.push({command: command, args: JSON.parse(args)});
        if (silent) return;
        replyKey = key;
        replyJson = JSON.stringify(command in answers ? answers[command] : answer);
        replyError = failure;
        replySerial++;
    }
}
