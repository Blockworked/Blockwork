# Blockwork for Nix. Built by the flake at the repo root; also usable with
# `callPackage` from an overlay.
{
  lib,
  rustPlatform,
  fetchurl,
  cmake,
  lld,
  autoPatchelfHook,
  libGL,
  pkg-config,
  qt6,
  dbus,
  libxkbcommon,
  gtk3,
  wayland,
}:

let
  cargoToml = lib.importTOML ../../Cargo.toml;
  qt = qt6.env "blockwork-qt" (with qt6; [ qtdeclarative qtsvg ]);
  # The archive blockwork-qt/native/CMakeLists.txt fetches; keep the two in step.
  qwindowkit = fetchurl {
    url = "https://codeload.github.com/stdware/qwindowkit/tar.gz/2072958b04820e9e7629f6c40068dee0ce0e9915";
    hash = "sha256-gs9Skrfzlyny1A9jzuNA3Xeuo8QC0E33NcI7dvmMgiw=";
  };
in
rustPlatform.buildRustPackage {
  pname = "blockwork";
  inherit (cargoToml.workspace.package) version;

  src = lib.cleanSource ../..;
  cargoLock = {
    lockFile = ../../Cargo.lock;
    # blockstitch, lucide-qt and enigo come from git at the revs Cargo.lock pins.
    allowBuiltinFetchGit = true;
  };

  nativeBuildInputs = [ cmake lld autoPatchelfHook pkg-config qt6.wrapQtAppsHook rustPlatform.bindgenHook ];
  buildInputs = [ qt qt6.qtbase qt6.qtwayland dbus libxkbcommon gtk3 wayland libGL ];

  # cmake is for QWindowKit, which build.rs drives itself.
  dontUseCmakeConfigure = true;
  # cargo-auditable links with gold, which, like ld with --as-needed, drops Qt
  # libraries only the later cxx-qt archives use. lld (rustup's default) doesn't,
  # but it bypasses Nix's ld wrapper, so autoPatchelfHook sets the RUNPATH.
  auditable = false;
  env.RUSTFLAGS = "-Clink-arg=-fuse-ld=lld";
  cargoBuildFlags = [ "--workspace" "--exclude" "blockwork-linux-bridge" ];
  doCheck = false;

  preBuild = ''
    export QMAKE=${qt}/bin/qmake
    mkdir -p "$NIX_BUILD_TOP/qwindowkit"
    tar -xzf ${qwindowkit} -C "$NIX_BUILD_TOP/qwindowkit" --strip-components=1
    export BLOCKWORK_QWINDOWKIT_SOURCE_DIR="$NIX_BUILD_TOP/qwindowkit"
    export CARGO_NET_OFFLINE=true
    # rustls' aws-lc-sys builds its own copy; nixpkgs' default asks for a system AWS-LC.
    unset AWS_LC_SYS_USE_SYSTEM
  '';

  installPhase = ''
    runHook preInstall
    patchShebangs packaging/linux/install.sh
    packaging/linux/install.sh "$out" "" "$(dirname "$(find target -path '*/release/blockwork' -type f -print -quit)")"
    # install.sh links /lib/blockwork/blockwork; point it into the store.
    ln -sf "$out/lib/blockwork/blockwork" "$out/bin/blockwork"
    runHook postInstall
  '';

  # Wrap both programs where they sit: the UI starts the daemon from beside itself.
  dontWrapQtApps = true;
  postFixup = ''
    wrapQtApp "$out/lib/blockwork/blockwork"
    wrapQtApp "$out/lib/blockwork/blockwork-daemon"
  '';

  meta = {
    description = "Build keyboard and mouse macros by snapping blocks together";
    homepage = "https://blockworked.com";
    license = lib.licenses.gpl3Only;
    mainProgram = "blockwork";
    platforms = [ "x86_64-linux" "aarch64-linux" ];
  };
}
