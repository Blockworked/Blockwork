import importlib.util
from pathlib import Path
import tempfile
import unittest


spec = importlib.util.spec_from_file_location(
    "prepare_release", Path(__file__).parents[1] / "prepare-release.py"
)
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)

CHANGELOG = """# Changelog
## [0.6.0] - Unreleased
### Added
- New feature.
### Fixed
- Fixed a bug.
## [0.5.4] - 2026-09-20
- Old release.
"""


class ReleasePreparationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.artifacts = Path(temporary.name)
        for _, filename, required in release.DOWNLOADS:
            if required:
                (self.artifacts / filename).write_bytes(b"package")

    def test_extracts_only_requested_release_with_or_without_v(self):
        for tag in ("0.6.0", "v0.6.0"):
            with self.subTest(tag=tag):
                body = release.release_body(CHANGELOG, tag, self.artifacts)
                self.assertIn("New feature.", body)
                self.assertIn("Fixed a bug.", body)
                self.assertNotIn("Old release.", body)

    def test_each_required_download_must_exist_and_be_nonempty(self):
        for _, filename, required in release.DOWNLOADS:
            if not required:
                continue
            path = self.artifacts / filename
            for empty in (False, True):
                with self.subTest(filename=filename, empty=empty):
                    if empty:
                        path.write_bytes(b"")
                    else:
                        path.unlink()
                    with self.assertRaisesRegex(ValueError, filename.replace(".", r"\.")):
                        release.release_body(CHANGELOG, "0.6.0", self.artifacts)
                    path.write_bytes(b"package")

    def test_optional_downloads_are_listed_only_when_available(self):
        for _, filename, required in release.DOWNLOADS:
            if required:
                continue
            path = self.artifacts / filename
            self.assertNotIn(filename, release.release_body(CHANGELOG, "0.6.0", self.artifacts))
            path.write_bytes(b"")
            self.assertNotIn(filename, release.release_body(CHANGELOG, "0.6.0", self.artifacts))
            path.write_bytes(b"package")
            self.assertIn(filename, release.release_body(CHANGELOG, "0.6.0", self.artifacts))

    def test_native_mac_builds_are_required_and_listed(self):
        required = {filename for _, filename, needed in release.DOWNLOADS if needed}
        body = release.release_body(CHANGELOG, "0.6.0", self.artifacts)
        for filename in ("blockwork-macos-arm64.app.zip", "blockwork-macos-x86_64.app.zip"):
            self.assertIn(filename, required)
            self.assertIn(filename, body)

    def test_unknown_version_does_not_publish_generic_notes(self):
        with self.assertRaisesRegex(ValueError, "No changelog section"):
            release.release_body(CHANGELOG, "0.6", self.artifacts)


if __name__ == "__main__":
    unittest.main()
