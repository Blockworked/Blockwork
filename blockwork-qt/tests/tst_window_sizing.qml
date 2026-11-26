import QtQuick
import QtTest
import com.blockworked.Blockwork 1.0

// Window parts read their `size` slot through WindowSizing.
TestCase {
    name: "WindowSizing"

    function test_axis_specs() {
        let axes = WindowSizing.specs("w:fill h:auto");
        compare(axes.w, "fill");
        compare(axes.h, "auto");
        axes = WindowSizing.specs("fill");
        compare(axes.w, "fill");
        compare(axes.h, "fill");
        axes = WindowSizing.specs("30% h:120");
        compare(axes.w, "30%");
        compare(axes.h, "120");
        axes = WindowSizing.specs("");
        compare(axes.w, "");
        compare(axes.h, "");
    }

    function test_measurements() {
        compare(WindowSizing.percent("30%"), 30);
        compare(WindowSizing.percent("120%"), 100);
        compare(WindowSizing.percent("auto"), -1);
        compare(WindowSizing.pixels("220"), 220);
        compare(WindowSizing.pixels("220px"), 220);
        compare(WindowSizing.pixels("auto"), -1);
        compare(WindowSizing.weight("2*"), 2);
        compare(WindowSizing.weight("fill"), -1);
    }
}
