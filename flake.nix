{
  description = "Blockwork: build keyboard and mouse macros by snapping blocks together";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAll (pkgs: rec {
        blockwork = pkgs.callPackage ./packaging/nix/package.nix { };
        default = blockwork;
      });

      overlays.default = final: _: { blockwork = final.callPackage ./packaging/nix/package.nix { }; };

      # programs.blockwork.enable = true; installs Blockwork, loads uinput and
      # lets the user at the seat use it. Recording still needs the input group.
      nixosModules.default =
        { config, lib, pkgs, ... }:
        let
          cfg = config.programs.blockwork;
        in
        {
          options.programs.blockwork = {
            enable = lib.mkEnableOption "Blockwork";
            package = lib.mkOption {
              type = lib.types.package;
              default = self.packages.${pkgs.stdenv.hostPlatform.system}.blockwork;
              description = "The Blockwork package to use.";
            };
          };
          config = lib.mkIf cfg.enable {
            environment.systemPackages = [ cfg.package ];
            services.udev.packages = [ cfg.package ];
            boot.kernelModules = [ "uinput" ];
          };
        };
    };
}
