import QtQuick
import QtTest
import com.blockworked.Blockstitch 1.0
import com.blockworked.Blockwork 1.0

Item {
    id: scene
    width: 1000; height: 800
    property var calls: []
    Item {
        id: page
        function invoke(command, args) { scene.calls = scene.calls.concat([{command: command, args: args || {}}]); }
    }
    RecordingReviewDialog { id: review; page: page }
    MockBridge { id: bridge; answer: ({os: "linux", capture: {available: true}}) }
    InputDiagnosticsDialog { id: diagnostics; appBridge: bridge }

    function pending() {
        return {
            macro_name: "Recorded",
            summary: {total: 6, keys: 2, buttons: 1, scrolls: 0, movements: 1, waits: 2, touch: 1, controller: 0, other: 0,
                      duration_ms: 3200, leading_wait_ms: 2500, longest_wait_ms: 2500, unreleased: 1},
            rows: [
                {source: "wait", direction: "", detail: "2500"},
                {source: "key", direction: "press", detail: "A"},
                {source: "touch", direction: "", detail: "2|340"},
                {source: "button", direction: "press", detail: "Left"}
            ]
        };
    }

    TestCase {
        name: "RecordingReview"
        when: windowShown
        function init() { scene.calls = []; }
        function cleanup() { review.close(); diagnostics.close(); wait(200); }

        function test_review_opens_with_sensible_defaults_and_inserts_the_choices() {
            review.show(scene.pending());
            tryCompare(review, "opened", true);
            // A long pause before the first input is skipped by default.
            verify(review.trimLeadingWait);
            verify(review.releaseHeld);
            compare(review.rowText(review.reviewData.rows[2]), "Touch with 2 finger(s) for 340 ms");
            compare(review.rowText(review.reviewData.rows[1]), "Press key A");
            review.keepMovement = false;
            review.capWaits = true;
            review.maxWaitSeconds = 2;
            review.insert();
            compare(scene.calls.length, 1);
            compare(scene.calls[0].command, "apply_recording_review");
            const options = scene.calls[0].args.options;
            compare(options.keepMovement, false);
            compare(options.keepKeys, true);
            compare(options.trimLeadingWait, true);
            compare(options.maxWaitMs, 2000);
            tryCompare(review, "opened", false);
        }

        function test_new_state_keeps_choices_and_discard_throws_it_away() {
            review.show(scene.pending());
            tryCompare(review, "opened", true);
            review.keepKeys = false;
            // A state update while open (e.g. a second recording) keeps the user's choices.
            const more = scene.pending();
            more.summary.total = 9;
            review.show(more);
            compare(review.keepKeys, false);
            compare(review.summary.total, 9);
            review.discard();
            compare(scene.calls[0].command, "discard_recording_review");
            tryCompare(review, "opened", false);
        }

        function test_diagnostics_ask_the_daemon_and_show_the_report() {
            bridge.calls = [];
            diagnostics.open();
            tryCompare(diagnostics, "opened", true);
            compare(bridge.calls[0].command, "input_diagnostics");
            tryVerify(() => diagnostics.report.indexOf("\"os\": \"linux\"") >= 0);
        }
    }
}
