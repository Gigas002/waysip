# Audit

A whole-application audit of waysip and libwaysip (2026-11-18) in five parts:
bugs (§2), performance (§3), security and robustness (§4), usability (§5), and
code quality, tests and project hygiene (§6). §7 lists what was left open, and
§8 is the checklist to re-run the audit.

The fixes are the 16 commits of branch `experimental/audit` on top of cebca5c
(`git log --reverse --stat cebca5c..experimental/audit`), one per finding,
followed by this document. Two of them change behaviour that scripts may
notice, and can be dropped on their own: 0177d99 (no mode flag now means
area selection) and 6fcc3af (a canceled selection exits with 1, like slurp).
No version or changelog was touched.

Each finding says what was found, how serious it is, and what was done:
**fixed** (with the commit), **accepted** (with the reason), or **OK**
(checked, nothing to do). Every fix is its own commit, with a regression test
where one can be written.

Severity: **high** (a wrong geometry or a feature that does nothing, in
normal use), **medium** (wrong behaviour in a reachable case, an ignored
setting), **low** (hardening, polish, or needs an unusual setup).

## 1. Method

Every source file of both crates was read in full. Findings were then
reproduced with release builds in a private headless sway 1.12
(`WLR_BACKENDS=headless`, pixman, its own runtime directory and D-Bus), never
on the desktop session.

Headless sway has no input devices, so a small helper created a
`zwlr_virtual_pointer_v1` and a `zwp_virtual_keyboard_v1` on its seat and
drove waysip with scripted moves, clicks, sweeps (1000 motions at 1 kHz) and
keys. Results were checked three ways: the printed geometry, `grim`
screenshots of the overlay, and the `WAYLAND_DEBUG` protocol log (redraws,
frame callbacks, objects created against objects destroyed). Scenarios:
every mode (`-d`, `-p`, `-o`, `-i`, `-d -o`, `-r`, `-a`, `-e`, `--freeze`),
drags in all four directions, one and two outputs (selections crossing
them), scale 2, output hot-plug and unplug during a selection, and input
devices coming and going.

Builds checked: default, `--no-default-features`, `--all-features`. Static
checks: `cargo clippy` at the three levels, `cargo deny check`, `typos`,
`cargo doc` with `-D warnings`, `cargo llvm-cov`.

## 2. Bugs

| Area | Finding | Status |
| ---- | ------- | ------ |
| Aspect ratio | `-a W:H` only shaped the rectangle drawn while dragging; the button release set the end corner to the raw pointer, so `-a 16:9` printed whatever was dragged (100x100 for a square drag). Dragging left or up also flipped the box to the right of the start point, as the ratio test compared signed sizes. Both go through one helper now; `-a 16:9` prints 177x100. A zero, negative or infinite ratio is rejected instead of dividing by zero. | Fixed, high (2dd9dd9) |
| No mode flag | Plain `waysip`, and `waysip -a 16:9`, `-e` or `-f ...` without `-d`, did nothing and exited 0. Area selection is the default now, as in slurp. | Fixed, high (0177d99) |
| Predefined boxes | The box height was `end_x - start_y`, and width and height mixed absolute and output-relative coordinates: a 400x200 box was drawn 400x400. Checked on screen. | Fixed, medium (e26ddeb) |
| `-r` with the pointer already on a box | Only pointer motion picked a box. Run from a keybind with the cursor over the window to pick, a click without moving canceled. Enter now picks the box too. | Fixed, medium (f82408b) |
| `-c` and `-s` | `-c` (border and text) set the selection fill and `-s` (selection) the border and text: `-c 00ff00ff` filled the selection green. | Fixed, medium (0d545ee) |
| Canceling | Esc printed "Selection canceled" and exited 0, so `waysip && grim -g ...` carried on with an empty geometry. It exits 1 now, like slurp. A behaviour change; it can be dropped on its own. | Fixed, medium (6fcc3af) |
| Size label | The label showed the drag's start corner and signed size ("500,500, -200x-100" for a drag up-left, while waysip printed "300,400 200x100"). Near the bottom-right edge it was drawn off the surface. It shows the printed geometry now, kept on the output that has the selection's corner, and the partial-redraw clip follows it so a moved label is cleared. | Fixed, low (f83a06c) |
| Box input | A label after the size (`x,y WxH title`, which slurp accepts and window-picker scripts emit) failed with "invalid float literal"; a blank line, such as a trailing one, failed the whole list. | Fixed, low (3754034) |
| `-i` output | Its last line had no newline and ran into the shell prompt. | Fixed, low (b42d4cf) |
| `AreaInfo::size_f` | Returned width and height swapped (library only). | Fixed, low (42005b0) |
| Edit mode, point, output, `-d -o` | Corner and body drags, Enter to confirm, Esc to abort, click vs drag in `-d -o`: as documented. | OK |
| Output hot-plug | An output added or removed during a selection neither panics nor breaks the selection on the others. | OK, see §7 |

## 3. Performance

Release builds, headless sway 1.12 (pixman), one output. "Drag" is 1000
pointer motions over one second while the button is held; CPU is waysip's
user+system time for it. Start-up is from `exec` to the first buffer attach,
median of 10. RSS is peak (`VmHWM`). slurp 1.5.0 was measured the same way.

| | cebca5c | experimental/audit | slurp |
| - | ------- | ------------------ | ----- |
| Redraws in the drag (`frame-limit`, the default) | 127 | 68 | — |
| Redraws in the drag (`--no-default-features`) | 1002 | 68 | — |
| Drag CPU, 1080p | 30–40 ms | 30 ms | 40–50 ms |
| Drag CPU, 4K | 120–140 ms | 90–100 ms | 220–240 ms |
| Drag CPU, 1080p / 4K, `--no-default-features` | 130–140 / 360–390 ms | 20–30 / 90–100 ms | — |
| Start-up, 1080p / 4K | 13.4 / 29.3 ms | 13.3 / 28.6 ms | 6.9 / 20.1 ms |
| Peak RSS, 1080p / 4K | 28.9 / 53.1 MB | 29.2 / 53.4 MB | 23.0 / 71.7 MB |
| `--freeze` start-up, 1080p | 62.3 ms | 54.6 ms | — |

| Area | Finding | Status |
| ---- | ------- | ------ |
| Redraw pacing | Every motion requested a frame callback on every surface and every callback redrew, so redraws followed the input rate (1002 for 1000 motions). The default `frame-limit` feature capped that with an 8 ms wall-clock throttle (127, still twice a 60 Hz display). Now one callback is outstanding at most and its redraw shows the latest state: 68 redraws, the output's 60 Hz, with no timer. `frame-limit` has no effect and is kept so builds that enable it keep working. At 4K without it, the old loop backed up the compositor far enough that the virtual pointer's socket returned `EAGAIN`. | Fixed, medium (3479dde) |
| `--freeze` conversion | The screenshot was copied (`to_rgba8`) and converted with per-pixel divisions. Taken as is and converted by pixel arrays, skipping the multiply for opaque pixels: 9.4 → 2.2 ms at 1080p, 34.8 → 7.0 ms at 4K, byte-identical output. The other ~45 ms of `--freeze` start-up at 1080p (~130 ms at 4K) is libwayshot's capture. | Fixed, low (fa5adf7) |
| Cursor theme | The xcursor theme was read from disk on every start and used only when the compositor lacks cursor-shape. Loaded only then: start-up 13.4 → 13.0 ms (with Adwaita), 180 KB less RSS. | Fixed, low (cf2b018) |
| HiDPI cost | On a scale-2 output the overlay now draws at full resolution (§5). That costs what the physical resolution costs: start-up 9.1 → 13.0 ms, RSS 23 → 29 MB, drag CPU 10–20 → 30 ms at 1920x1080 scale 2. | Accepted |
| Start-up vs slurp | waysip is 6–9 ms slower to its first frame. In a protocol-logged run (which slows it), about half went to three registry and output roundtrips (the registry is listed twice) and half to painting the first full-screen buffer (8 MB at 1080p, page faults included). | Accepted, see §7 |

## 4. Security and robustness

| Area | Finding | Status |
| ---- | ------- | ------ |
| Objects left on the connection | A selection left a `wl_shm_pool` per buffer (each mapping a full-screen buffer in the compositor), the xdg outputs, and the pointer and keyboard; every seat capability change created another pointer and keyboard. The final destroys were never flushed. With a caller-kept connection (`WaySip::with_connection`) all of it stayed on the server, and the overlay stayed mapped until the caller's next flush. Pools are destroyed once their buffer exists, devices follow their capability, a teardown destroys the rest and a roundtrip delivers it before `get` returns. Protocol log: pools, buffers, xdg outputs, pointers and keyboards now balance. `wl_output`s are kept because `ScreenInfo` hands them out. | Fixed, medium (1c483c6) |
| Output unplug and device churn | Unplugging either of two outputs mid-selection, and input devices disappearing and coming back, leave a working selection. | OK |
| Input validation | Bad colors, border weight, aspect ratio and box input fail with a message and exit 1, never a panic. The integration tests cover the first three; box input is covered by unit tests of its parser. | OK |
| Dependencies | `cargo deny check` passes. | OK |

## 5. Usability

| Area | Finding | Status |
| ---- | ------- | ------ |
| HiDPI | The overlay buffer had the logical size and no buffer scale, so on a scale-2 output it was stretched: a 2 px blurred border, blocky text, and a frozen screenshot at half resolution. It follows `wl_output.scale` now (fractional scales get the compositor's rounded-up integer scale). | Fixed, medium (3500db6) |
| Box color | The default box color (`#66666680`) is the default background, so `-r` boxes are invisible until `-B` is given. | Accepted, see §7 |
| Geometry vs slurp | A drag from pixel 101 to pixel 1500 prints 1399 wide, slurp 1400: slurp includes the pixel under the cursor. Kept, since scripts may rely on waysip's numbers. | Accepted |
| Relative geometry across outputs | For a selection crossing outputs, `%X/%Y/%W/%H` are relative to the output where the pointer was released, and only clipped on its right and bottom. | Accepted |
| README | Notes that `-d` is the default and that `frame-limit` no longer does anything. | Fixed, docs (0177d99, 3479dde) |

## 6. Code quality, tests and hygiene

| Area | Finding | Status |
| ---- | ------- | ------ |
| Regression tests | Every fix above has one: dispatch tests on inert proxies (aspect ratio, enter over a box, seat devices, frame pacing, output scale), pixel tests on real cairo surfaces (box sizes, label placement and clearing, scaled drawing), a live test against the CI compositor (teardown), CLI tests (ratio validation, cancel exit code, colors). The new tests passed 24 parallel runs, and the live one 20, with no failure. None depends on wall-clock time. | Fixed |
| Coverage | 76.3 % → 79.0 % of lines (`cargo llvm-cov --workspace --all-features`, run under headless sway so the live tests run). `lib.rs`'s event loop (35 %) and `main.rs` (33 %) need real input and were checked end to end with the virtual pointer instead. | Fixed |
| Cargo warnings | `workspace.package.description` was inherited by no crate, and `tracing` was unused without `logger`; every build warned. | Fixed (32f11f6) |
| Test isolation | The CLI integration tests remove `WAYLAND_DISPLAY`; wayland-rs has no `wayland-0` fallback, so they can't reach a desktop compositor. | OK |
| Gates | fmt, typos, deny, clippy and release builds at the three feature levels, and the tests under headless sway, pass on every commit. | OK |

## 7. Left open

- **Overlay on hot-plugged outputs.** An output added during a selection gets
  no overlay, so the selection can't be drawn there. Handling it needs the
  surfaces created per output on `global` events, not only at start-up.
- **Layer namespace.** The overlay's layer-shell namespace is `"osk"` (on-screen
  keyboard), so compositor rules for keyboards apply to it and rules for
  waysip can't name it. `"waysip"` would be right, but it breaks existing
  user rules written against `"osk"`.
- **Default box color.** Making `-B` default to something other than the
  background makes `-r` boxes visible; it is a visible design choice.
- **Start-up roundtrips.** The registry is listed twice (`registry_queue_init`
  and a second `get_registry` for outputs). Binding outputs from the first
  list and requesting their xdg outputs right away would save a roundtrip.
- **Aspect ratio while editing.** With `-a` and `-e`, dragging a corner handle
  doesn't keep the ratio.
- **`frame-limit`.** Now a no-op; it can be removed in a release that may
  break builds naming it.

## 8. Re-running the audit

```sh
# Gates (in a session with a compositor, or headless sway as in CI)
cargo fmt --all -- --check && typos && cargo deny check
for f in --no-default-features "" --all-features; do
  cargo clippy --workspace --all-targets $f -- -D warnings
  cargo build --workspace --release $f
done
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features

# Coverage (system LLVM tools on Arch: LLVM_COV / LLVM_PROFDATA)
LLVM_COV=/usr/bin/llvm-cov LLVM_PROFDATA=/usr/bin/llvm-profdata \
  cargo llvm-cov --workspace --all-features --summary-only

# Redraws per drag (expect about one per output frame)
WAYLAND_DEBUG=1 waysip 2> log   # drag for a second, release
grep -c 'wl_surface@[0-9]*.attach' log
```

For the end-to-end checks, run sway with `WLR_BACKENDS=headless` in its own
`XDG_RUNTIME_DIR`, and drive waysip with a `zwlr_virtual_pointer_v1` and a
`zwp_virtual_keyboard_v1` client. `swaymsg seat ... cursor` does nothing
there, because headless sway has no pointer device.
