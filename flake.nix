# Matty for Nix: `nix run github:karyalabs/matty`. The package is the
# release's Linux build (nix/package.nix), x86_64 only for now.
{
  description = "Matty, a native Mattermost client";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAll (pkgs: rec {
        matty = pkgs.callPackage ./nix/package.nix { };
        default = matty;
      });

      overlays.default = final: _prev: {
        matty = final.callPackage ./nix/package.nix { };
      };

      formatter = forAll (pkgs: pkgs.nixfmt);
    };
}
