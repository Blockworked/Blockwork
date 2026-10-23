cask "blockwork" do
  version "__VERSION__"
  sha256 "__CHECKSUM__"

  url "https://github.com/Blockworked/Blockwork/releases/download/__TAG__/blockwork-macos-arm64.app.zip"
  name "Blockwork"
  desc "Visually build and run keyboard and mouse macros"
  homepage "https://github.com/Blockworked/Blockwork"

  depends_on arch: :arm64

  app "Blockwork.app"

  # The app is only ad-hoc signed (no Developer ID), so drop the quarantine flag
  postflight do
    system_command "/usr/bin/xattr",
                   args: ["-dr", "com.apple.quarantine", "#{appdir}/Blockwork.app"]
  end

  zap trash: [
    "~/Library/Application Support/Blockwork",
    "~/Library/Caches/blockwork",
  ]

  caveats <<~EOS
    Blockwork is not notarized by Apple. This cask removes the quarantine
    attribute after install so it can launch. It also needs Accessibility and
    Input Monitoring permissions in System Settings > Privacy & Security.
  EOS
end
