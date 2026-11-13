# Upstream pins

Matty pins its UI stack exactly. These versions move together: gpuicn is
built and tested against one GPUI Kit release, which in turn pins one
`gpui-pre` runtime. **Recheck before each milestone and never upgrade
silently**: a pin change is its own pull request that updates this table.

| Dependency | Pin | Source of truth | Checked |
| --- | --- | --- | --- |
| Rust toolchain | 1.99.1 (`rust-toolchain.toml`; MSRV 1.98) | gpuicn `release-pins.toml` `[rust]` | 2026-11-03, Claude for Arya |
| gpuicn | `=0.11.0` (tag `v0.11.0`), crate mode, `syntect` on native | crates.io `gpuicn` | 2026-11-10, Claude for Arya |
| GPUI Kit | `gpui-kit =0.9.1` (`39ab849`) | gpuicn `release-pins.toml` `[gpui_kit]` | 2026-11-10, Claude for Arya |
| GPUI runtime | `gpui-pre-platform =0.3.12` (`gpui-pre` `d2a4fb6`) | gpuicn `release-pins.toml` `[gpui]` | 2026-11-10, Claude for Arya |
| GPUI fork | `gpui-pre`, `gpui-pre-apple` and `gpui-pre-macos` from `karyalabs/zed` branch `crates/gpui-pre-0.3.12-perf` (`b5147ca`), through `[patch.crates-io]` in `Cargo.toml` | `karyalabs/zed` `karya/README.md`; gpui-av `docs/spec.md` 4.3 | 2026-11-13, Claude for Arya |
| gpui-icons | `=0.7.0` (Lucide 1.63.0) | gpuicn `release-pins.toml` `[gpui_icons]` | 2026-11-10, Claude for Arya |
| IBM Plex Sans and Mono fonts | Google Fonts static instances (Plex Sans v23, Plex Mono v20; Mono Medium given the typographic family name "IBM Plex Mono" so it joins its family), `crates/matty-app/assets/fonts` | Google Fonts API, OFL | 2026-11-07, Claude for Arya |
| Web toolchain | `nightly-2026-08-17` with `rust-src` (atomics, `-Z build-std`), wasm-bindgen CLI = Cargo.lock's `wasm-bindgen`, binaryen 132 | gpuicn `release-pins.toml` `[wasm]`; `scripts/web` | 2026-11-04, Claude for Arya |

Gaps Matty reported in gpuicn, and their status, are listed in [`gpuicn-gaps.md`](gpuicn-gaps.md).

## How gpuicn is consumed

`crates/matty-app` uses gpuicn in **crate mode** (`gpuicn.toml`,
`mode = "crate"`): `src/ui/mod.rs` re-exports the `gpuicn` crate, so code
reads `ui::button::Button` and so on. A component is ejected (`gpuicn eject
<name>`) only to fix a bug that is also reported upstream; Matty never keeps a
local substitute for a gpuicn component.

Import GPUI types through `gpui_kit` so Matty and gpuicn share one set of GPUI
types.

## Karya Labs' GPUI fork

GPUI comes from Karya Labs' fork, `karyalabs/zed`, by default. The fork
publishes gpui-pre-compatible crates (same names, same versions) on the
branch `crates/gpui-pre-<version>`, holding only the crates its patches
change; `[patch.crates-io]` in the root `Cargo.toml` swaps those in and every
other crate stays on crates.io. The patches keep GPUI's API (today 10-bit and
HDR video surfaces on macOS, and redraws scoped to the views a click, tooltip
or focus change affects, with `App::set_nested_view_caching` as the one new
call), so gpuicn and GPUI Kit build unchanged, and each one is also offered
upstream to Zed and dropped once it lands there.
`deny.toml` allows this one git source. The plan of record is the fork's
`karya/README.md` and gpui-av `docs/spec.md` section 4.3.

## Changing a pin

1. Read the gpuicn release notes and its `release-pins.toml` for the new
   release.
2. Update `[workspace.dependencies]` in `Cargo.toml`, `rust-toolchain.toml`
   and the CI toolchain together, then this table with the date and who
   checked.
3. When the `gpui-pre` version moves, move the fork with it before this PR:
   on `karyalabs/zed`, start `gpui-pre/<new version>` at the new snapshot's
   `zed-rev`, rebase the fork's patches onto it, run
   `karya/publish.sh <new version>` to generate `crates/gpui-pre-<new version>`,
   then point the `[patch.crates-io]` entries at that branch and update the
   fork row above. A patch that no longer applies blocks the bump; never drop
   it silently.
4. Run the full CI locally (`cargo clippy`, `cargo test`, `scripts/check-no-gpui.sh`)
   and attach before/after screenshots when anything visible changes.
