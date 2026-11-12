# Matty for Nix, from the Linux tarball of the release named in
# release.json. The Release workflow updates release.json after each
# release (docs/releasing.md); nothing here builds from source.
{
  lib,
  stdenv,
  fetchurl,
  autoPatchelfHook,
  makeWrapper,
  libgcc,
  libxcb,
  libxkbcommon,
  libGL,
  vulkan-loader,
  wayland,
}:

let
  release = lib.importJSON ./release.json;
in
stdenv.mkDerivation {
  pname = "matty";
  inherit (release) version;

  src = fetchurl {
    url = "https://github.com/karyalabs/matty/releases/download/v${release.version}/Matty-${release.version}-linux-x86_64.tar.gz";
    inherit (release) hash;
  };
  sourceRoot = ".";

  nativeBuildInputs = [
    autoPatchelfHook
    makeWrapper
  ];
  # What the binary links.
  buildInputs = [
    libgcc
    libxcb
    libxkbcommon
  ];

  dontConfigure = true;
  dontBuild = true;

  installPhase = ''
    runHook preInstall
    mkdir -p $out
    cp -r usr/. $out/
    runHook postInstall
  '';

  # GPUI opens these at run time rather than linking them, so autoPatchelf
  # can't see them. The graphics driver itself comes from the system
  # (hardware.graphics on NixOS).
  postFixup = ''
    wrapProgram $out/bin/matty \
      --prefix LD_LIBRARY_PATH : ${
        lib.makeLibraryPath [
          libGL
          vulkan-loader
          wayland
        ]
      }
  '';

  meta = {
    description = "Unofficial native desktop client for Mattermost";
    homepage = "https://github.com/karyalabs/matty";
    changelog = "https://github.com/karyalabs/matty/releases/tag/v${release.version}";
    license = lib.licenses.mit;
    sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
    platforms = [ "x86_64-linux" ];
    mainProgram = "matty";
  };
}
