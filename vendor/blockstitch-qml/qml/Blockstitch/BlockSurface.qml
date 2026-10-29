import QtQuick
import QtQuick.Shapes

// Paints a block silhouette: a flat fill and an outline.
// "stack" / "header" / "cap" are single rows; "wrap" is a C-shaped block
// (head bar, one mouth per body, mid bars between bodies, foot bar) whose inner
// connector notches/tabs line up with the blocks nested in its mouths.
// A Shape rather than a Canvas: a canvas per block is a texture per block,
// repainted on the CPU on every hover, and blurred by the canvas zoom.
Shape {
    id: root
    property string shape: "stack"
    property color fill: Theme.block
    property color outline: Theme.border
    property color hoverColor: Theme.accent
    property bool hovered: false
    property real outlineWidth: 1.3

    // wrap geometry: bar *body* heights (the bottom connector tab protrudes
    // `tab` px below the foot bar) and one entry per mouth
    property real headHeight: 50
    property real midHeight: 34
    property real footHeight: 26
    property real spine: 20
    property var mouthHeights: []
    property bool headNotch: true   // wrap: top connector notch
    property bool footTab: true     // wrap: bottom connector tab (false for an ending block / reporter)
    property var flatEnds: []   // per mouth: last nested block has no bottom tab, so the bar below it has no notch
    readonly property real tab: 8

    preferredRendererType: Shape.CurveRenderer

    // connector profile, relative to the connector's left edge: [x, depth]
    readonly property var profile: [[0, 0], [8, 8], [22, 8], [30, 0]]

    function connector(ctx, x0, y, reverse) {
        const rounding = 1.5;
        const points = reverse ? profile.slice().reverse() : profile;
        const direction = reverse ? -1 : 1;
        for (let i = 0; i < points.length; ++i) {
            const point = points[i];
            const before = i > 0 ? points[i - 1] : [point[0] - direction, point[1]];
            const after = i + 1 < points.length ? points[i + 1] : [point[0] + direction, point[1]];
            const incoming = Math.hypot(before[0] - point[0], before[1] - point[1]);
            const outgoing = Math.hypot(after[0] - point[0], after[1] - point[1]);
            ctx.lineTo(x0 + point[0] + (before[0] - point[0]) * rounding / incoming,
                       y + point[1] + (before[1] - point[1]) * rounding / incoming);
            ctx.quadraticCurveTo(x0 + point[0], y + point[1],
                                 x0 + point[0] + (after[0] - point[0]) * rounding / outgoing,
                                 y + point[1] + (after[1] - point[1]) * rounding / outgoing);
        }
    }

    function notch(ctx, x0, y) { connector(ctx, x0, y, false); }
    function tabTo(ctx, x0, y) { connector(ctx, x0, y, true); }

    function trace(ctx) {
        const ins = .6, r = 5, w = width, h = height;
        // Shared horizontal edges must land on the same line when blocks interlock.
        const L = ins, R = w - ins, T = 0;
        if (shape !== "wrap") {
            const topNotch = shape !== "header", bottomTab = shape !== "cap";
            const base = bottomTab ? h - tab : h;
            ctx.moveTo(L + r, T);
            if (topNotch) { notch(ctx, 13, T); }
            ctx.lineTo(R - r, T); ctx.quadraticCurveTo(R, T, R, T + r);
            ctx.lineTo(R, base - r); ctx.quadraticCurveTo(R, base, R - r, base);
            if (bottomTab) { tabTo(ctx, 13, base); }
            ctx.lineTo(L + r, base); ctx.quadraticCurveTo(L, base, L, base - r);
            ctx.lineTo(L, T + r); ctx.quadraticCurveTo(L, T, L + r, T);
            ctx.closePath();
            return;
        }
        const S = spine, mouthLeft = S + ins, mouths = mouthHeights || [];
        ctx.moveTo(L + r, T);
        if (headNotch) { notch(ctx, 13, T); }
        ctx.lineTo(R - r, T); ctx.quadraticCurveTo(R, T, R, T + r);
        let y = headHeight;
        ctx.lineTo(R, y - r); ctx.quadraticCurveTo(R, y, R - r, y);
        tabTo(ctx, S + 13, y);
        ctx.lineTo(mouthLeft + r, y);
        ctx.quadraticCurveTo(mouthLeft, y, mouthLeft, y + r);
        for (let k = 0; k < mouths.length; ++k) {
            const foot = k === mouths.length - 1;
            y += mouths[k];
            // Match the nested block's left corners so the canvas cannot show through.
            ctx.lineTo(mouthLeft, y - r);
            ctx.quadraticCurveTo(mouthLeft, y, mouthLeft + r, y);
            if (!(flatEnds || [])[k]) { notch(ctx, S + 13, y); }
            ctx.lineTo(R - r, y); ctx.quadraticCurveTo(R, y, R, y + r);
            y += foot ? footHeight : midHeight;
            ctx.lineTo(R, y - r); ctx.quadraticCurveTo(R, y, R - r, y);
            if (foot) {
                if (footTab) { tabTo(ctx, 13, y); }
                ctx.lineTo(L + r, y); ctx.quadraticCurveTo(L, y, L, y - r);
            } else {
                tabTo(ctx, S + 13, y);
                ctx.lineTo(mouthLeft + r, y);
                ctx.quadraticCurveTo(mouthLeft, y, mouthLeft, y + r);
            }
        }
        ctx.lineTo(L, T + r); ctx.quadraticCurveTo(L, T, L + r, T);
        ctx.closePath();
    }

    // The outline as SVG path data, traced through a tiny canvas-like writer.
    function writer() {
        const parts = [];
        const n = v => Math.round(v * 100) / 100;
        return {
            moveTo: (x, y) => parts.push("M" + n(x) + " " + n(y)),
            lineTo: (x, y) => parts.push("L" + n(x) + " " + n(y)),
            quadraticCurveTo: (cx, cy, x, y) => parts.push("Q" + n(cx) + " " + n(cy) + " " + n(x) + " " + n(y)),
            closePath: () => parts.push("Z"),
            text: () => parts.join(" ")
        };
    }
    readonly property string outlinePath: {
        if (width <= 0 || height <= 0) return "";
        const w = writer();
        trace(w);
        return w.text();
    }

    ShapePath {
        strokeColor: root.hovered ? root.hoverColor : root.outline
        strokeWidth: root.outlineWidth
        joinStyle: ShapePath.RoundJoin
        fillColor: root.fill
        PathSvg { path: root.outlinePath }
    }
}
