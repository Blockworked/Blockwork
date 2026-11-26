pragma Singleton
import QtQuick

// Reads a window part's size hint, the text a `size` slot holds:
// "w:fill h:fill", "w:30%", "w:220", "w:auto h:120". A bare token is the
// width; "fill" alone fills both ways.
QtObject {
    function specs(text) {
        const out = { w: "", h: "" };
        for (const token of String(text ?? "").trim().toLowerCase().split(/\s+/)) {
            if (!token) continue;
            const at = token.indexOf(":");
            if (at > 0) {
                const axis = token.slice(0, at), spec = token.slice(at + 1);
                if (axis === "w" || axis === "h") out[axis] = spec;
                else if (axis === "both") { out.w = spec; out.h = spec; }
            } else if (token === "fill") {
                out.w = "fill";
                out.h = "fill";
            } else {
                out.w = token;
            }
        }
        return out;
    }
    // "30%" -> 30, else -1.
    function percent(spec) {
        if (!spec.endsWith("%")) return -1;
        const value = parseFloat(spec);
        return Number.isFinite(value) ? Math.max(0, Math.min(100, value)) : -1;
    }
    // "220" or "220px" -> 220, else -1.
    function pixels(spec) {
        if (!/^\d+(\.\d+)?(px)?$/.test(spec)) return -1;
        const value = parseFloat(spec);
        return Number.isFinite(value) ? value : -1;
    }
    // "2*" -> 2, else -1.
    function weight(spec) {
        if (!spec.endsWith("*")) return -1;
        const value = parseFloat(spec);
        return Number.isFinite(value) && value > 0 ? value : -1;
    }
}
