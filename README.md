# Tempest 2032

A [Tempest 2000](https://en.wikipedia.org/wiki/Tempest_2000) clone with graphics for modern hardware.

The goal is to make Tempest 2000 easy to install and play on today's computers,
preserve its exact gameplay, and update the graphics for modern GPUs and
high-resolution displays.

[![Animated gameplay on level 8, with a gold claw, particle laser, enemies, and explosion rings](assets/gameplay.gif)](assets/steam/gameplay.png)

## Differences from the original

These are the deliberate differences from the original:

- **Resolution.** The game renders at your display's native resolution, in any
  window size and aspect ratio, well beyond 4K. The original drew a 384×240
  framebuffer. Antialiasing smooths geometry and steps down automatically if
  the machine cannot keep the simulation at 60 FPS, and touch devices skip the
  most expensive setting from the start.
- **More visible warp gates.** Gate flight shows three solid gates and three
  more that fade with distance, reducing sudden pop-in on high-resolution displays.
- **Sound effects overlap.** While the music played, the original could mix only
  two effects, so busy scenes cut each other off. The effect draws from a
  pool of sixteen voices, so explosions and announcements stay audible.
- **One welcome screen.** The original cycled through title, attract, and
  level-select screens. Here one screen carries the high-score table, the player
  Key and starting web, and the settings, over a slowly rotating wireframe of
  the selected web. High scores, Keys, and difficulty are kept separately for
  Low, Normal, and High, with six initials slots.
- **Missing modes.** Only Tempest 2000 mode is implemented. Traditional
  Tempest, Tempest Plus, Tempest Duel, the earned Beastly mode, and the attract
  demo are not, and neither is rotary controller support.

Anything else that differs from the original is a bug, not a decision. Pull
requests that restore original behavior are welcome.

## Install and play

**[Play in your browser](https://tempest2032.github.io/)** with touch controls, a keyboard, or a controller.

- **Android:** use the browser menu's **Install app** or **Add to Home screen** option.
- **iPhone / iPad:** in Safari, use **Share → Add to Home Screen**, with **Open as
  Web App** enabled if offered. iOS support is expected but has not been tested.

Launch the installed game from its icon to open Welcome directly, without the
browser's Play prompt. Sound may start on your first touch. The app hides browser
controls; fullscreen depends on the platform. After the game finishes caching,
it also works offline.

Download your platform's package from [GitHub Releases](https://github.com/tempest2032/tempest2032/releases).
Then:

- **Windows:** run the setup `.exe`. It installs for your account and adds a Start
  menu shortcut. No administrator access is required.
- **macOS, Apple Silicon:** open the `.dmg`, drag **Tempest 2032** into
  **Applications**, and open it from there.
- **Linux, x86_64 or arm64:** extract the `.tar.gz`, copy its folder anywhere,
  and run `tempest2032`. The tarball needs glibc 2.39 or newer and system audio,
  display, and graphics libraries.
- **Linux Flatpak:** open the `.flatpak` bundle in your software center, or run
  `flatpak install --user ./tempest2032-linux-x86_64.flatpak`, using the arm64
  bundle on arm64 machines. Launch **Tempest 2032** from your applications menu.

The macOS app and Windows installer are not publisher-signed yet. On macOS,
use **Open Anyway** in System Settings → Privacy & Security if Gatekeeper blocks
the downloaded app.

Windows shortcuts, the macOS app, and Flatpak start in fullscreen.
All game assets and levels are included. Linux needs a graphical desktop and a
Vulkan-capable graphics driver.

Note: Development builds are available under **Artifacts** in successful
[Application packages workflow](https://github.com/tempest2032/tempest2032/actions/workflows/packages.yml)
runs for three days.

### Build from source

Install [Rust](https://rustup.rs/) 1.89 or newer and run from the source directory:

```sh
cargo run --release
```

The first build takes a while. To build a standalone executable:

```sh
cargo build --release
```

You'll find it at `target/release/tempest2032`, or
`target/release/tempest2032.exe` on Windows. It runs without the source tree.

On Debian or Ubuntu, install the build dependencies first:

```sh
sudo apt install build-essential pkg-config libasound2-dev libudev-dev \
  libx11-dev libxi-dev libxrandr-dev libxcursor-dev \
  libxkbcommon-dev libxkbcommon-x11-0 libwayland-dev
```

### Add to Steam

Choose **Install in Steam** in the welcome screen's configuration menu, then
restart Steam. You can also close Steam and run:

```sh
cargo run --release -- --steam-install
```

This installs a standalone copy and adds it to your Steam library as a non-Steam
game, with artwork and fullscreen launch options. Open Steam to see it. Supports
Steam Deck and native, Flatpak, and Snap Steam installations.

## Controls

Controller buttons use Xbox names.

On touch devices, slide your thumb in the left control area to steer, and lift
to stop. Use Fire, Jump, and Zapper on the right; hold Fire to keep shooting.
Slide up/down in warp stages and menus, including high-score initials entry.
Menus use OK and Back. Landscape puts the controls over the game;
portrait gives them a separate strip below it. Controls appear automatically
on touch devices. Slide from Fire onto Jump or Zapper to use them without lifting
your finger. Tap the playfield to pause, then tap Resume, Restart, or Exit.
Settings is in the top-left corner of Welcome. Buttons give short vibration
feedback where the browser supports it.

| Action | Keyboard | Controller |
| --- | --- | --- |
| Move | Left/right arrows or A/D | D-pad or left stick |
| Fire | Hold Z or Space | Hold A |
| Jump, once unlocked | X | B, left trigger, or left bumper |
| Superzapper | C | X, right trigger, or right bumper |
| Pause/resume | P or Pause | Start |
| Toggle fullscreen | F | Keyboard only |
| Show/hide FPS | F3 | — |
| Restart | Restart in the pause menu | Restart in the pause menu |
| Return to Welcome | Esc during gameplay or Exit in the pause menu | Exit in the pause menu |
| Quit | Esc on Welcome | B on Welcome |

Use up/down and confirm with Enter, Space, or controller A in the pause menu.
Esc resumes when paused. In warp stages, steer with the arrows, W/A/S/D, D-pad,
or left stick.
In gate-flight warps, release the vertical control after an arrow gate to coast
on its boost. Holding that direction at the height limit stops the momentum.
Enter confirms menus and does not fire. In Steam Input, use a Gamepad layout
to keep the same controller controls on Steam Deck and other Steam controllers.

For high-score initials, type three letters or use left/right to select a slot
and up/down to change its letter. Enter / A saves; Backspace / B erases.
On Steam Deck, open the virtual keyboard with Steam + X.
Initials follow your OS keyboard layout; gameplay bindings use physical key positions.
While entering initials, F types the letter instead of toggling fullscreen.

## Options

```sh
cargo run --release -- --fullscreen
cargo run --release -- --level 17 --difficulty easy
cargo run --release -- --volume 0.3 --music-volume 0.5
cargo run --release -- --help
```

Difficulty initially defaults to `normal`, then remembers your menu selection.
`--difficulty` overrides it for that launch. `--level` and `--warp-zone` skip the
welcome screen and disable high-score saving, including `--level 1`.
Optional TOML files in `levels/` override the embedded levels; changed definitions
also disable high-score saving.

To make your own levels and choose music files or URLs, start with the
[custom-level demo and options guide](examples/custom-level/).

## Development

```sh
cargo fmt --check
cargo check
cargo test
cargo clippy -- -D warnings
```

The [reference guide](docs/reference.md) covers level customization, capture
tools, asset generation, and gameplay and rendering details.

## About this project

Project-authored code, documentation, and artwork are licensed under
[GNU GPLv3](LICENSE). Imported Tempest 2000 assets are excluded from that grant;
their redistribution permission has not been established. See [NOTICE](NOTICE)
for the affected files, third-party attribution, and mixed or derived material
that still needs review.
Original source reference: [mwenge/tempest2k](https://github.com/mwenge/tempest2k).

Built mostly with OpenAI's Sol 6.1 at medium reasoning effort, with some work
by Space Bunny at high reasoning effort.

2032 is a nod to Tempest 2000, which was released in 1994, six years before
the year in its name. Tempest 2032 follows the same convention for 2026.
