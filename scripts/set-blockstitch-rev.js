const fs = require("node:fs");
const path = require("node:path");

const revision = process.argv[2];
if (process.argv.length !== 3 || !/^[A-Za-z0-9][A-Za-z0-9._/-]*$/.test(revision ?? "")) {
    console.error("Usage: node scripts/set-blockstitch-rev.js <revision>");
    process.exit(1);
}

const manifest = path.join(__dirname, "..", "Cargo.toml");
const contents = fs.readFileSync(manifest, "utf8");
const pinned = new Set();
const updated = contents.replace(
    /(blockstitch-(?:core|qml|ffi)\s*=\s*\{[^}]*?\brev\s*=\s*")[^"]+(")/g,
    (match, prefix, suffix) => {
        pinned.add(/blockstitch-(core|qml|ffi)/.exec(prefix)[1]);
        return prefix + revision + suffix;
    },
);
if (!pinned.has("core") || !pinned.has("qml")) {
    console.error("Expected revision pins for both blockstitch-core and blockstitch-qml in Cargo.toml");
    process.exit(1);
}
fs.writeFileSync(manifest, updated);

// The flatpak's blockstitch source must match; cargo-sources.json still needs
// regenerating (`just flatpak`'s generator) after a full commit hash change.
const flatpak = path.join(__dirname, "..", "packaging", "flatpak", "com.blockworked.Blockwork.yml");
if (fs.existsSync(flatpak) && /^[0-9a-f]{40}$/.test(revision)) {
    const yml = fs.readFileSync(flatpak, "utf8");
    fs.writeFileSync(
        flatpak,
        yml.replace(/(url: https:\/\/github\.com\/Blockworked\/blockstitch\.git\n\s*commit: )[0-9a-f]+/, "$1" + revision),
    );
}
