# Matty on Nix and NixOS

Matty has a Nix flake at the root of this repository. It installs the same
Linux build as the `.deb`, the `.rpm` and the AppImage: the tarball from
the latest [release](https://github.com/karyalabs/matty/releases), with its
libraries pointed at the Nix store so it runs on NixOS.

## Try it

```sh
nix run github:karyalabs/matty
```

This needs flakes turned on (`experimental-features = nix-command flakes`
in `nix.conf`).

## Install it

**For your user**, with `nix profile`:

```sh
nix profile add github:karyalabs/matty      # Nix before 2.30: nix profile install
```

**On NixOS**, add the flake to your system flake and the package to your
system or Home Manager packages:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    matty = {
      url = "github:karyalabs/matty";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, matty, ... }: {
    nixosConfigurations.my-machine = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        ./configuration.nix
        {
          environment.systemPackages = [ matty.packages.x86_64-linux.default ];
          # Or, with Home Manager: home-manager.users.<you>.home.packages
        }
      ];
    };
  };
}
```

There is an overlay too, if you prefer `pkgs.matty`:

```nix
nixpkgs.overlays = [ matty.overlays.default ];
environment.systemPackages = [ pkgs.matty ];
```

## What your system needs

- **x86_64 Linux.** Releases have no ARM Linux build yet, so the flake has
  none either.
- **Graphics drivers.** Matty draws with Vulkan through GPUI. On NixOS,
  `hardware.graphics.enable = true;` (the default on any desktop setup)
  provides the drivers. Without a GPU driver Matty does not start.
- **A Secret Service** (GNOME Keyring, KWallet, KeePassXC) to keep you
  signed in. Without one Matty still works, but asks you to sign in at each
  launch.
- **Wayland or X11.** Both work; Matty picks Wayland when it is there.

The package adds Matty's desktop entry and icon, so it shows up in your
launcher and opens `matty://` and `mattermost://` links (the "Open in
desktop app" handoff from your server's sign-in page) once your desktop
picks up the entry.

**Nix on another distribution** (Ubuntu, Fedora, Arch and so on) works the
same way, but the Nix build can't use your distribution's graphics drivers
on its own. Use the `.deb`, the `.rpm` or the AppImage there instead, or
run the Nix build through [nixGL](https://github.com/nix-community/nixGL).

## Updates

Matty does not update itself on Linux, with any package; that includes
Nix. In **Settings → General**, **Check for updates** says "This build
doesn't update itself" and offers the releases page. Nix manages the
version instead:

| You installed with | To update |
| --- | --- |
| `nix run github:karyalabs/matty` | Nothing: it fetches the latest each time (Nix may cache it for an hour; add `--refresh` to skip that). |
| `nix profile add` | `nix profile upgrade matty` |
| A NixOS or Home Manager flake | `nix flake update matty`, then rebuild (`nixos-rebuild switch` or `home-manager switch`) |

A self-updating app could not replace itself in the read-only Nix store
anyway, so this is how it stays.

**How soon a release reaches Nix.** The Release workflow
([releasing.md](releasing.md)) publishes a release, then opens a pull
request that points the flake at it. That pull request builds the package,
runs it, and merges itself, usually within a few minutes. The flake follows
every release, betas included, just as the README's downloads do. The same
pull request moves the flake's own nixpkgs pin forward, so the libraries
Matty runs with stay current.

## Support

Nix is supported on the same footing as the other Linux packages: each
release is built for it and checked to start. Report problems as a
[GitHub issue](https://github.com/karyalabs/matty/issues) and say that you
use Nix, with your NixOS version or distribution, and Wayland or X11.

Not there yet:

- **nixpkgs.** Matty is not in nixpkgs, so `nix-env -iA nixpkgs.matty` and
  `pkgs.matty` without the overlay don't work. Adding it is planned once
  Matty has a stable release; nixpkgs prefers to build from source, which
  the next item makes possible.
- **Building from source.** The flake repackages the release binary rather
  than compiling Matty, so installs are quick. A source build is planned;
  it would also bring ARM Linux.
- **ARM Linux** (aarch64), as above.

## How it works

| File | What it is |
| --- | --- |
| [`flake.nix`](../flake.nix) | The flake: `packages.x86_64-linux.default` (also `.matty`), `overlays.default`, and `formatter` (nixfmt). |
| [`flake.lock`](../flake.lock) | The nixpkgs it builds against. |
| [`nix/package.nix`](../nix/package.nix) | The package: fetches the release tarball, patches the binary's interpreter and linked libraries with `autoPatchelfHook`, and puts the libraries GPUI opens at run time (Vulkan, Wayland, EGL) on its `LD_LIBRARY_PATH`. |
| [`nix/release.json`](../nix/release.json) | The release it installs: version and the tarball's hash. |
| [`scripts/nix-release`](../scripts/nix-release) | Writes `release.json` from a release's `SHA256SUMS`. |

The `nix` job in [`release.yml`](../.github/workflows/release.yml) runs
`scripts/nix-release`, `nix flake update`, `nix build` and
`matty --version`, then opens the pull request with karya-karta's token and
turns on auto-merge. CI's code filter leaves out `flake.nix`, `flake.lock`
and `nix/`, so that pull request runs no jobs. If the job fails, the
release is still out; run the same steps by hand:

```sh
gh release download v<version> --pattern SHA256SUMS
scripts/nix-release <version> SHA256SUMS
nix flake update
nix build .#matty && ./result/bin/matty --version
```
