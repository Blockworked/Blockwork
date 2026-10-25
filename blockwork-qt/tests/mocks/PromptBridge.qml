import QtQml
QtObject {
    property string requestJson: "{}"
    property bool parentRunning: true
    function parentAlive() { return parentRunning; }
    signal finished(string answer, string text)
    function finish(answer, text) { finished(answer, text); }
    function bringToFront() {}
}
