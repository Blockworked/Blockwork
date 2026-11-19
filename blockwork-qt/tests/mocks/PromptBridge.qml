import QtQml
QtObject {
    property string requestJson: "{}"
    property bool parentRunning: true
    function parentAlive() { return parentRunning; }
    signal finished(string answer, string text)
    function finish(answer, text) { finished(answer, text); }
    signal sent(string answer, string text, string values)
    function sendValues(answer, text, values) { sent(answer, text, values); }
    function bringToFront() {}
}
