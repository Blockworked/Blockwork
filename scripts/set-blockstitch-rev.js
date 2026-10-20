const fs = require("node:fs");
const path = require("node:path");

const revision = process.argv[2];
if (process.argv.length !== 3 || !/^[A-Za-z0-9][A-Za-z0-9._/-]*$/.test(revision ?? "")) {
    console.error("Usage: node scripts/set-blockstitch-rev.js <revision>");
    process.exit(1);
}

const manifest = path.join(__dirname, "..", "Cargo.toml");
const contents = fs.readFileSync(manifest, "utf8");
let replacements = 0;
const updated = contents.replace(
    /(blockstitch-(?:core|qml)\s*=\s*\{[^}]*?\brev\s*=\s*")[^"]+(")/g,
    (match, prefix, suffix) => {
        replacements += 1;
        return prefix + revision + suffix;
    },
);
if (replacements !== 2) {
    console.error("Expected revision pins for both blockstitch-core and blockstitch-qml in Cargo.toml");
    process.exit(1);
}
fs.writeFileSync(manifest, updated);
