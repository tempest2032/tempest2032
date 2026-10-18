# Tempest 2032 reference guide

Implementation details, customization instructions, and research notes about
the original game. For installation and basic controls, see the
[README](../README.md), which also lists the deliberate differences from the
original. Run commands from the repository root.

## Overview

Tempest 2032 uses Rust and Bevy 0.18.
The first level uses the original game's irregular 16-sided `web5` outline
and straight extrusion. Per-lane shaded surfaces fade into the tunnel, and yellow
edges mark the glider's current lane. The solid gold glider uses the original
`sclaw0` through `sclaw7` vertex layouts, with shaded faces and a thin extruded
body. The claw stays at each lane's midpoint while fractional movement progress
selects its rolling pose in the direction of movement. It holds its pose when
stopped.

An offset perspective camera follows half the claw's displacement at a capped
speed, so it continues settling after the claw stops. A sparse colored
starfield fills the background. Geometry is generated in code from editable
level definitions. Gate flight embeds the original Jaguar CRY surface texture;
no external asset directory is needed at runtime.
Normal stars use the web camera's full world-space projection, so their
expansion point, the web's depth rails, and the exit flight share one vanishing
point. Web coordinates retain the original `(8, 8)` origin; asymmetric shapes
and the tracking camera can place the finite web opening away from the screen
center. The camera does not recenter each web by its bounding box.
Star quads are generated without per-star heap allocations, and animated web
surfaces update their existing color buffers in place. Mesh builders reserve
space for known triangle counts and keep collapsed triangles' normals finite.

Horizontal plane webs, including level 3's `web1`, use the original half-claw
viewpoint and 30-unit front distance. Their lens follows the original
128-pixel projection scale, with extra room in narrow windows. Sideways yaw
is disabled on every web so horizontal rim edges, including the triangle's
base, remain parallel to the viewport as the claw moves.

## Run

Install Rust 1.89 or newer, then run from this directory:

```sh
cargo run
```

The first build compiles Bevy and takes longer than subsequent builds.
For an optimized build, use `cargo run --release`.
To build the standalone executable, use `cargo build --release`; it is written
to `target/release/tempest2032` on Linux and macOS, or
`target/release/tempest2032.exe` on Windows.
Release builds strip symbols, use thin link-time optimization with one codegen
unit, and abort on panic. These settings reduce executable size while retaining
speed-focused optimization, at the cost of longer builds and less crash
diagnostic information. Development builds retain their normal diagnostics.

Press `P` or the keyboard's `Pause` key to pause
and resume. Pausing freezes gameplay, animations, timers, and audio, and shows
the original-style red-and-gold PAUSED lettering over the frozen scene.
The pause menu offers Resume, Restart, and Exit, with Resume selected each time
it opens. Use up/down arrows or W/S to choose, then Enter or Space to confirm.
Click or tap the playfield to pause; click or tap a pause-menu choice to activate
it directly. The hit regions follow the lettering's viewport-scaled layout.
Restart returns to level one with a fresh score and lives.
Exit abandons the current run without saving its score and returns to Welcome.
Press `P` or `Pause` again to resume.
`Esc` resumes while paused and returns to Welcome during gameplay, abandoning
the run without saving its score. Press Esc on Welcome to quit.
On the game-over panel, press Enter, Space, or controller A to continue. A
qualifying standard run asks for three-letter initials before returning to
Welcome, whether it earned a high score or a player Key. Key-started runs save
under their existing initials; other runs return directly. Held buttons do not
dismiss the panel.
Run `cargo run -- --help` to list command-line options, or `--version` to show
the version. Screenshot and recording modes are mutually exclusive; recordings
accept an optional positive duration in whole seconds after the directory.
Pass `--fullscreen` to start borderless fullscreen on the primary monitor
instead of a window. Press F to switch between a window and borderless fullscreen
on the current monitor. This shortcut is keyboard-only and works in menus and
while paused; during initials entry, F remains a letter input.
Pass `--fps` to start with the frame rate in the top-right corner.
Press F3 to show or hide the display at any time, including menus and while paused.
The label refreshes every 0.5 seconds and shows the
fastest and slowest frame times in milliseconds over the last two seconds.
Every frame counts, so a single hitch appears at the next refresh and stays
in the range until it ages out. The same line reports this process's average
CPU usage as a percentage of total CPU capacity and its resident memory in
MiB, once their first readings arrive. It also reports the current antialiasing
step as `AA full`, `AA fxaa`, or `AA off`.

Presentation uses FIFO VSync to avoid tearing. With `--fps`, late application
updates also produce warnings when their interval exceeds 1.5 display refresh
periods, or 25 ms at 60 Hz. The threshold follows the window's current monitor;
if its refresh rate is unavailable, the log states that 60 Hz is assumed.
Warnings are limited to once per second and include the number of late
intervals and the worst interval. Unfocused, hidden, and minimized windows
are ignored, as is the first interval after resuming or changing refresh rate.
These warnings indicate possible missed refreshes. Bevy does not expose actual
display presentation timing, so they cannot confirm or detect every dropped
frame. Simulation runs at 60 Hz without render interpolation.

On Linux, the build needs a C/C++ toolchain and X11 and Wayland development
libraries. On Debian or Ubuntu, install `build-essential`, `pkg-config`,
`libx11-dev`, `libxi-dev`, `libxrandr-dev`, `libxcursor-dev`,
`libxkbcommon-dev`, `libxkbcommon-x11-0`, `libwayland-dev`, `libudev-dev`, and
`libasound2-dev`. Running needs a
graphical desktop and a Vulkan-capable graphics driver. Windows and macOS use
their native window backends.

## Browser deployment

Play at <https://tempest2032.github.io/>. The `Browser deployment` workflow builds
the browser game on pushes to `main` and manual dispatches from `main`. It installs
the WASM target and the bindings CLI version from `Cargo.lock`, then runs
`python3 tools/web.py build` with the optimized `web-release` profile.

The workflow publishes `target/web/`, including LICENSE and NOTICE, to the
`gh-pages` branch of `tempest2032/tempest2032.github.io`. That repository serves
the branch's root through GitHub Pages, with Jekyll disabled. Alongside the page
it publishes `assets/preview.png` as `preview.png`, cropped from the Steam
gameplay screenshot, which link unfurlers such as Slack use for a site preview.
The build tags the page and the loader with a hash of everything it produced,
including their own templates, because the game calls into the loader and a
cached loader from an older build breaks the newer game. The source stays in
`tempest2032/tempest2032`. The `Tempest 2032 Pages` GitHub App, installed
only on the website repository with Contents write access, authenticates
publishing. The publish job exchanges the app's private key for a token that is
limited to that repository and expires after an hour. The token cannot change
workflow files there. The app's client ID and private key are the
`PAGES_APP_CLIENT_ID` variable and `PAGES_APP_PRIVATE_KEY` secret of the source
repository's `website` environment, which only accepts deployments from `main`.
To rotate the key, generate a new one in the app settings, replace the
environment secret, then delete the old key. A ruleset on the website repository
blocks deleting or force-pushing `gh-pages`, and only repository admins can
change `main`.

Building and publishing are separate jobs. The build job compiles third-party
crates and uses the Rust cache but has no access to the app credentials. It hands
`target/web/` to the publish job as a workflow artifact. The publish job runs no
repository code and restores no cache, so a malicious build script or poisoned
cache entry cannot read the app key.

## Local browser build

### Install on a phone

Open <https://tempest2032.github.io/> in Chrome on Android, then choose **Install
app** or **Add to Home screen** from the browser menu and confirm installation.
Launch Tempest 2032 from its home-screen or app-drawer icon. The manifest requests
fullscreen, including hiding the status and browser bars on supported Android
browsers. Browsers that fall back to a standalone window still hide the page's
fullscreen/GitHub footer. Portrait and landscape both remain available. Opening
the website as a normal browser tab retains its usual footer and Fullscreen button.

On iPhone or iPad, open the site in Safari and choose **Share → Add to Home
Screen**. Leave **Open as Web App** enabled if offered, then launch from the new
icon. iOS opens home-screen web apps in standalone mode rather than honoring the
manifest's Android-style fullscreen request. System status or home indicators may
remain visible. The page includes Apple's web-app metadata, a touch icon, and
safe-area-aware touch controls. Recent Safari versions support the WebAssembly,
WebGL2, Web Audio, and service-worker APIs used here, but iOS gameplay, audio,
and offline launching have not been tested on a device. Safari does not support
the vibration feedback used by the touch controls.

Installed launches detected through the fullscreen, standalone, or minimal-ui
display modes, or iOS's `navigator.standalone`, open Welcome as soon as the game
is ready. They skip the HTML Play prompt without starting a run. Audio resume is
attempted immediately; if autoplay policy blocks it, normal touches anywhere in
the player or keyboard input retry it without consuming the input. Returning
after suspension still requires Resume, so a backgrounded run cannot restart
unexpectedly. Normal browser tabs retain the Play prompt.

The first online visit installs a service worker and caches the complete game,
including WASM, levels, music, and effects. Once that cache has finished installing,
the installed game and browser page can launch offline. Player data still uses
the same origin's `localStorage`. Cache storage is separate from scores and Keys.
PWA installation and service workers require HTTPS, or localhost for development;
a plain HTTP LAN address can run the game but cannot install its offline worker.

The generated worker caches one complete build under a content-hashed cache name.
Navigation, including launch-option queries, uses that build's cached page, and
the page, loader, touch module, stylesheet, and WASM all use matching versioned
URLs. A new build downloads in the background and waits for all existing game
tabs and installed-game windows to close before activating. Reopen the game to
use the update. Activation removes the previous game cache without changing
player storage. Service-worker or storage failures leave normal online play
available. The worker only handles same-origin requests inside its deployment
scope, so builds also work below a URL subdirectory.

Regenerate the committed installation icons from `logos/favicon.svg` with
`uv run tools/web_icons.py`. Browser builds use those bundled PNGs without image
conversion dependencies. Run `python3 -m unittest discover -s tools -p test_web.py`
to check manifest icons, build versioning, and precache URLs. Run
`node --test tools/test_browser.mjs` to check installed startup and audio unlocking.

### Build and serve

The browser version uses Bevy's WebGL2 renderer and a single-threaded
`wasm32-unknown-unknown` build. All levels, effects, music, and other game assets
remain embedded. It requires a WebGL2-capable browser and supports touch controls,
a keyboard, or a controller.

Install the WASM target and a `wasm-bindgen-cli` version matching the
`wasm-bindgen` package in `Cargo.lock`. The current version is 0.2.129:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
make web
make web-serve
```

Open <http://127.0.0.1:8000> and click Play to enable audio and focus the canvas.
The canvas fills the window above a small fullscreen/GitHub bar. Controls appear
in the Play/Resume prompt.
Enter / controller A then starts a run from Welcome. The Fullscreen button uses
the browser's fullscreen API, as does F while the game canvas has focus. F types
the letter during initials entry. Press a controller button to let the browser detect
it. Esc on Welcome leaves the game running; close the tab to quit.

Touch controls appear automatically when the browser reports a coarse pointer or
touch support, including in fullscreen. The Play/Resume prompt shows touch
instructions on those devices, and keyboard instructions otherwise. Portrait
reserves a 190-pixel strip below the canvas; landscape overlays translucent
controls near the bottom edges. Controls
respect the browser's safe-area insets. Touch anywhere in the left slide area,
then move your thumb left/right to steer. Hold the displacement to keep moving,
return to the touch origin or lift to stop. Up/down slides steer in warp stages,
select menu choices, and edit initials. Fire becomes OK in menus and Jump
becomes Back; Zapper is hidden there. Fire/OK stays in the bottom-right corner,
with Zapper directly above and Jump/Back directly to its left.
An action finger can slide between these buttons;
crossing the gaps keeps its last action held until it reaches another button.
Short visits to Jump or Zapper survive between polls. Tap the playfield to pause,
then tap Resume, Restart, or Exit on the existing menu lettering. Touch controls
do not trigger playfield taps. Settings sits in the top-left corner on Welcome
and Configuration. There is no separate pause button. Canvas pointer coordinates
are mapped to the fitted window size so pause-menu hit regions match the lettering
on high-DPI displays too.

Buttons and steering directions request a 4 ms vibration on activation and
on release through `navigator.vibrate`, including action-button slides. Steering
vibrates when a direction engages, changes, or returns to neutral, without
repeating while held steady. Entering or leaving the pause
menu requests the same 4 ms pulse. Input resets do not vibrate. Feedback is best-effort;
unsupported browsers and contexts continue without it.

`web/touch.js` tracks one steering thumb independently of the action fingers.
Steering has a 10-CSS-pixel dead zone around the touch origin and caps each axis
at 32 pixels, following longer slides so reversal does not require crossing the
whole area. Captured gestures continue outside the slide area's border. The
latest steering direction replaces pending directions, avoiding opposite inputs
from a quick reversal between polls. Short gestures and button taps survive
between input polls. It passes an action bitmask
through `tempestBrowser.touchInput` in `src/browser.rs`. The controller mapper
merges touch and physical controller actions before updating the virtual keys,
ahead of menu, pause, and fixed gameplay input. Screen or pause-context changes
clear touches so held OK cannot fire into a new run or dismiss game over.
Pointer cancellation, suspension, resize, rotation, and capability changes clear
touch state too. Touch changes neither movement rates nor the 60 Hz simulation.

The build uses the `web-release` Cargo profile, with optimization level 2 and the
native release profile's LTO settings. `tools/web.py` checks the bindings version,
removes the release WASM's debugging name section, and generates the complete
static site in `target/web/`, including license and
attribution files. No server-side game code is needed. The local server binds only
to all interfaces, allowing a phone on the same LAN to reach it. These commands
also work without Make:

```sh
python3 tools/web.py build
python3 tools/web.py serve --port 8000
```

For development, `python3 tools/web.py build --profile dev` uses the normal Cargo
development profile. The page accepts `fps`, `aa`, `level`, `warp-zone`,
`warp-course`, `warp-gate`, `volume`, and `music-volume` URL parameters, for example:

```text
http://127.0.0.1:8000/?fps
http://127.0.0.1:8000/?level=80&fps
http://127.0.0.1:8000/?warp-zone=tunnel&warp-course=2
http://127.0.0.1:8000/?aa=fxaa&fps
```

F3 toggles the FPS display in the browser too; `?fps` makes it visible at launch.
`?aa=auto`, `full`, `fxaa`, or `off` overrides the starting antialiasing step.
Explicit level and warp starts bypass Welcome and disable score submission, as
their native command-line counterparts do. The browser FPS display retains frame
timing but omits desktop process CPU/memory readings and monitor-refresh warnings.
Filesystem level overrides, capture tools, and Steam installation are native-only.

The same validated player TOML is stored in `localStorage` under the key
`tempest2032/player.toml`. Scores and difficulty are local to the browser and
origin, so `localhost`, `127.0.0.1`, different ports, and the hosted site have
separate data. Clearing site data removes the tables. Invalid stored data is
preserved and disables saving for that session; storage failures show a notice.

Losing page or canvas focus, or hiding the tab, suspends virtual time and the audio
context. Click Resume to continue. An existing gameplay pause stays paused after
resuming the page. Pending action inputs are cleared, and the 60 Hz simulation
does not catch up for the time spent away. Rendering and audio callbacks share the
browser thread, so expensive rendering or shader compilation can interrupt audio.
Hardware-accelerated Firefox and Chromium have been exercised locally. Chromium's
software renderer was too slow for smooth playback on the test machine. Safari and
physical controller compatibility still need manual testing; standard controller
input has been checked through a simulated browser Gamepad API.

## Welcome, settings, high scores, and Keys

Normal launches open Welcome with level two's colored star field drifting outward
behind the menu panel. Welcome, Configuration, and initials entry share this
real-time background, independent of the gameplay stars. Welcome places the
centered high-score table over a large rotating wireframe of the selected web,
directly over the stars without a separate box or a dimming overlay.
Left/right arrows, D-pad, or left stick select a player Key or a fresh run.
Up/down arrows, W/S, D-pad, or left stick select an odd-numbered starting web.
The selected web's deferred score award appears as `6000 BONUS`, for example,
below the level selector. Web 1 shows `0 BONUS`; the amount updates with the web.
Fresh runs can start on webs 1 through 15; a Key starts at its saved maximum
and can select any earlier odd web. Selection defaults to web 1 for a fresh
run or the selected Key's maximum, and resets to web 1 when returning from a
run or changing difficulty. P, Pause, or controller Start opens Configuration.
Use up/down to select, left/right to change
difficulty, and Enter / A to confirm. P / Start or Esc / B returns to Welcome.
Esc / B on Welcome quits.

Welcome and initials entry share the embedded spectrum-portal logo, the
announcements' beveled lettering, and the HUD's rainbow score digits.
Both render at the UI's native resolution. Welcome's transparent layout occupies
90% of the window width; the centered initials panel occupies 70%. Both use
86% of the height, with lettering, padding, and spacing scaled to the window.
Initials entry highlights the
selected letter with rainbow shading and a gold underline, keeping the fields
above the Steam Deck keyboard's lower-half overlay.

The web preview uses the original vertices, lane connections, open ends, and
straight extrusion from `Web::vertex`, including the configured tunnel depth.
Its front rim uses `appearance.lines`, exactly as gameplay does; rails fade
toward a darker rear rim without changing hue or blending the whole web into
the background. Thus repeated shapes remain distinguishable by level color.
A stationary perspective camera fits the wireframe's complete rotational sweep
to a full-window viewport, using up to 90% of the shorter window axis. It looks
parallel to the tunnel, aligning its vanishing point with the screen-centered
starfield. For flat webs, it moves slightly above the plane to reveal the depth
without tilting the view. The score lettering renders above the web. Preview
framing may recenter the mesh for this purpose; gameplay retains its original
web origin and tracking camera.
The preview and the score lettering render straight into the window, sharing
its multisampled targets instead of allocating offscreen images. The preview
camera clears the window to black behind every menu screen and draws the menu
stars, built like the gameplay star field's one-pixel quads, and on Welcome the
web. The lettering camera follows the canvas node's rectangle and draws after
the UI, so the overlay camera stays the default UI camera. While a menu is
shown, the gameplay cameras are inactive and the overlay camera renders only
the UI, not its HUD layer.
The preview camera carries `OverlayCamera` and its own render layer, so playing,
retrying, warping, or returning to Welcome cannot move it. Geometry is rebuilt
only when the selected web or viewport size changes. The web makes one full
turn every twenty seconds around its own depth axis, without orbiting or moving
the camera. Rotation follows real time, continues smoothly across level choices,
and stops outside Welcome. Long frame gaps are capped at 0.1 seconds, matching
the menu stars. The level-two starfield stays independent of selection.

Gameplay fixed updates stop on front-end screens, while real-time star
animation, input, and title music remain live. Gameplay pause still freezes
virtual time and audio. Transition buttons are consumed, and held controller
buttons or keyboard auto-repeat cannot confirm the next screen or fire into
the new run.

The local top-ten tables are separate for Low, Normal, and High. Equal scores
stay behind earlier entries; zero scores do not qualify. Only completed runs
started from Welcome with definitions matching the entire bundled catalog can
save scores. Identical external definitions count as standard; modified or
additional levels do not. Explicit `--level`, including `--level 1`, direct
`--warp-zone`, screenshots, recordings, and message previews bypass Welcome
and disable score and Key saving for the entire session, including after
restarting. Abandoned runs do not submit scores or Keys. Key starts selected on
Welcome remain eligible for both.

### Player Keys

Six three-letter identities each store independent Low, Normal, and High
checkpoints. This extends the original four-Key limit and keeps progress earned
on Low from unlocking starts on Normal or High. The Welcome selector displays
only Keys with a checkpoint for the current difficulty. Entering the same
initials updates that identity without reducing any saved level. At capacity,
new initials replace the Key with the lowest checkpoint at the selected
difficulty, only when the new checkpoint is higher. Replacement removes that
identity's checkpoints on all three difficulties; a missing checkpoint counts
as zero when selecting the lowest slot.

A run records the highest web whose exit it survives. Game over rounds this
down to an odd starting web from 17 through 97. Dying on web 23 after finishing
22 therefore saves web 21; failing the exit through spikes does not complete
the web. Successful warps also unlock the webs they skip; a failed warp adds
no progress. Checkpoints survive retries and the existing level-100 loop;
Restart starts a fresh run on web one.
The 97 cap follows `gameover`'s maximum `cwave = 98` and `setkey`'s subtract-one,
clear-low-bit encoding (`yak.s:2779-2791`, `yak.s:5743-5751`).

On game over, a fresh run that qualifies for a Key can enter initials even when
it misses the top ten. If both qualify, one initials submission saves both.
A Key-started run automatically credits its score and updates its checkpoint
under the selected initials, with a confirmation on Welcome.

All new runs initialize zero score and three spare lives (`yak.s:1079-1084`);
`setlives` draws those spares and `it` ends the game only below zero
(`yak.s:18319-18328`, `yak.s:8469-8472`). Here, `Game::lives` includes the active
ship, so that initial state is four total lives. This applies to ordinary starts,
Key starts, direct `--level` starts, and restarts. None grants score or a 1UP
during the initial launch animation.

Level-selection confirmation arms `startbonus` with **+1** (`yak.s:1833-1838`).
Initial startup follows `it` → `setweb`/`sweb` → `stdinit` → `rotate_web`
(`yak.s:8475-8479`, `yak.s:8533-8563`), bypassing `sweb0`. Both wave generators
test the flag's sign and skip the 1UP while it is nonnegative
(`yak.s:17809-17812`, `yak.s:17903-17906`). Arming the flag is therefore not an
immediate award.

The first completed-web transition schedules `sweb0` (`yak.s:13966-13968`).
After any intervening bonus stage, it adds the selected starting web's bonus
directly to the earned score and changes `startbonus` to **-1**
(`yak.s:8897-8962`). The bonus comes from `slebon`: with zero-based starting-web
index `n`, it is `n * (2600 + 200 * n)` (`yak.s:9072-9079`). The next wave's
generator then grants exactly one 1UP and clears the flag. Adding this score
bypasses score-based life awards. For example, a web-31 start launches with zero
score and three spares; its first completed-web transition adds 258,000 points
to the earned score and arms one 1UP for the following web. A web-1 start has a
zero-point bonus, with the same delayed 1UP. Retries before that transition keep
the bonus armed; retries and further completions after it do not repeat it.
Acquired upgrades and warp tokens start empty.

### Initials and storage

Initials entry accepts A-Z from OS-translated logical keyboard characters,
falling back to text events, and also accepts IME commits. It follows the active
OS layout rather than physical QWERTY key positions used for gameplay, and
converts lowercase letters to uppercase. On a controller, left/right selects a slot,
up/down cycles its letter, A saves, and B erases. Physical keyboards use
Backspace and Enter. Steam Deck's manually opened Steam + X keyboard uses the
same field. The last submitted initials are reused as the next default.

Difficulty, initials, score tables, and Keys are stored in `tempest2032/player.toml`
under `dirs::data_local_dir()`: `$XDG_DATA_HOME` or `~/.local/share` on Linux,
`%LOCALAPPDATA%` on Windows, and `~/Library/Application Support` on macOS.
This path does not depend on the executable's location. Writes use a temporary
file in the same directory and atomic replacement. Failed writes keep scores
in memory and display an error. An unreadable, invalid, or unsupported-version
file is preserved, with saving disabled for that session. Version-one files
migrate in memory to version two, retaining scores, difficulty, and initials;
the next save writes the new format. Key levels are stored in Low/Normal/High
order, using zero for an unearned checkpoint. Invalid levels, duplicate Key
initials, and more than six identities disable saving rather than being discarded.

Capture front-end screens without playing a run:

```sh
cargo run -- --menu-preview welcome --message-size 1280x800 --screenshot target/welcome.png
cargo run -- --menu-preview welcome-key --screenshot target/welcome-key.png
cargo run -- --menu-preview configuration --screenshot target/configuration.png
cargo run -- --menu-preview initials --screenshot target/initials.png
cargo run -- --menu-preview key-initials --screenshot target/key-initials.png
```

`--menu-preview` requires `--screenshot` and cannot combine with level, warp,
or message previews. `--message-size` accepts either kind of preview.
`welcome-key` selects the first available Key, supplying an in-memory web-31
example if needed; `key-initials` previews earning a Key without a high score.
Neither preview writes player data.

## Continuous integration

GitHub Actions builds all Cargo targets and runs the tests on Windows x86_64,
macOS arm64, Linux x86_64, and Linux arm64 using native runners and stable Rust.
Checks run on pushes to `main`, pull requests, and manual workflow dispatches.
They also check Rust formatting, Linux and WASM Clippy lints, and the
Python packaging/build and JavaScript browser tests. The tagged-release workflow
reuses these checks before building and publishing packages.
Cargo downloads and compiled dependencies are cached separately for each target,
Rust version, and build configuration. Default-branch pushes populate the caches;
pull requests reuse them. Dependency changes reuse compatible cached artifacts.

The separate zizmor workflow audits GitHub Actions security on pushes, pull
requests, manual dispatches, and daily at 04:17 UTC. To run the same
check locally, install uv and run `make zizmor`. The target uses `uvx zizmor .`
with a fresh, isolated environment to pick up the latest zizmor release each time.
Set `GH_TOKEN` to enable its online audits; local checks work without a token.
The workflows use read-only repository permissions and pinned action commits,
and do not persist checkout credentials.

## Application packages

`.github/workflows/packages.yml` runs on pushes to `main`, pull requests, and manual
dispatches. It builds optimized packages on native runners for Windows x86_64,
macOS arm64, and Linux x86_64/arm64. Each job uploads its packages as an Actions
artifact with a three-day retention period. It is also a reusable workflow called
by the release workflow for tag builds.
Package jobs launch the Windows installation, signed macOS app bundle, staged
Linux executable, and installed Flatpak outside the source tree. Each must render
Welcome and level-one screenshots and exit successfully within 120 seconds per
scene. Linux uses Xvfb and Mesa's software Vulkan driver. The `smoke-*` Actions
artifacts retain screenshots and process logs for three days, including failed
launch logs. These checks exercise startup and rendering, but do not establish
clean-Windows compatibility, audible output, or physical GPU/controller support.
Native checks are enabled with `tools/package.py --smoke-test`; the shared runner
can also be called as `python3 tools/smoke.py --output target/smoke -- /absolute/path/to/tempest2032`.
Windows shortcuts and the installer's Play action pass `--fullscreen`. The macOS
app and Flatpak use a small launcher that passes `--fullscreen` and forwards any
additional arguments to the game executable.

### Tagged releases

Pushing a tag triggers `.github/workflows/release.yml`. The tagged commit must
contain the release workflow. Before tagging a version such as `v0.1.0`, update
the version in `Cargo.toml` and the AppStream release metadata so the package
filenames and application metadata match the release.

The release workflow first runs the four-platform build/test workflow, then
calls the package workflow. Once all checks and package builds pass, it creates
a GitHub release for the tag with generated release notes and attaches:

- The Windows installer and macOS DMG.
- Linux tarballs and Flatpak bundles for x86_64 and arm64.
- One architecture-independent Flathub submission archive.

The release is prepared as a draft and published after all uploads succeed.
Rerunning the workflow updates the assets of the existing release. Only the
publishing job has repository write permission. Release assets remain available
beyond the three-day expiry of the intermediate Actions artifacts.

### Native packages

`tools/package.py` packages the standalone binary:

- Windows uses the runner's Inno Setup 6 installation and
  `packaging/windows/installer.iss`. Installation is per-user, with a Start menu
  entry, an optional desktop shortcut, upgrades in place, and an uninstaller.
  Release executables use the Windows GUI subsystem and embed the application icon.
  The C runtime is linked statically. CI silently installs the package into a
  temporary folder, checks its executable, and runs its uninstaller.
- macOS creates a versioned `.app` with an icon and bundle metadata, applies an
  ad-hoc signature, and places it beside an Applications symlink in a compressed
  DMG. No installer or administrator access is needed. Publisher signing and
  notarization are not configured.
  CI verifies the app signature and DMG checksum before uploading.
- Linux archives contain the executable, icon, and short usage instructions.
  They are built on Ubuntu 24.04 and require glibc 2.39 or newer. Flatpak is the
  alternative for distributions with older system libraries.

The icon files are committed so builds need no image-conversion dependencies.
Regenerate them with `uv run tools/package_icons.py`.

For example, after a native release build on Linux:

```sh
python3 tools/package.py linux --target x86_64-unknown-linux-gnu \
  --binary target/release/tempest2032 --output target/packages
```

### Flatpak and Flathub

The Flatpak jobs build `io.github.tempest2032.Tempest2032` for x86_64 and aarch64
inside the Freedesktop 25.08 SDK using its Rust extension. Build commands run
offline. `tools/package.py flatpak` generates crate download sources and checksums
from `Cargo.lock`, plus a source manifest pinned to the workflow's commit. The
current generator supports crates.io dependencies and rejects other sources.
The local CI manifest builds the checked-out tree, including pull-request changes.

The sandbox allows display, audio, GPU, and controller access. Device access uses
`--device=all` for compatibility with the Flatpak version on Ubuntu 24.04; no
host-filesystem access is requested. Network access allows custom soundtrack
downloads. Steam installation is not exposed inside the sandbox.

Each job uploads a `.flatpak` bundle and `flathub-submission.tar.gz`, containing
the pinned source manifest and `cargo-sources.json`. CI installs the built bundle
and checks that its executable starts inside the sandbox. For Flathub submission,
extract these files into the proposed Flathub application repository and follow
[Flathub's submission process](https://docs.flathub.org/docs/for-app-authors/submission/).
Use a commit available in the upstream repository, rather than a pull-request
merge commit. Flathub publication is separate from building bundles in Actions.
Cargo and AppStream declare `GPL-3.0-only`. Packages include `LICENSE` and
`NOTICE`, which record third-party attribution and original-material provenance.
Update the AppStream release metadata when changing the Cargo package version.

To build a bundle locally, install Flatpak, flatpak-builder, and the matching
Freedesktop runtime, SDK, and Rust extension, then run:

```sh
python3 tools/package.py flatpak --revision FULL_COMMIT_ID --output target/flatpak-package
flatpak-builder --user --force-clean --repo=target/flatpak-repo \
  target/flatpak-build target/flatpak-package/local.json
flatpak build-bundle --runtime-repo=https://flathub.org/repo/flathub.flatpakrepo \
  target/flatpak-repo target/tempest2032.flatpak io.github.tempest2032.Tempest2032
```

## Steam installation

The welcome screen's configuration menu also offers **Install in Steam**. It
runs the same installer on a background thread and displays success or error
details without blocking menu input. The success message advises restarting
Steam and warns when the running client might overwrite the shortcut. This
action is unavailable inside the game's Flatpak sandbox.

Run the executable you want to install:

```sh
tempest2032 --steam-install
```

For a local optimized build, use `cargo run --release -- --steam-install`.
The command copies the executable into a stable per-user directory, adds Tempest 2032
as a non-Steam game for the most recently used Steam account, prints the paths
and shortcut ID, and exits without opening the game. If Steam is still running,
the command warns and continues; Tempest 2032 appears in your library after the next
Steam start, and Steam can drop the shortcut again if it keeps overwriting the
file in the meantime. The initial launch option is `--fullscreen`.

Native Steam is detected on Linux, Steam Deck, Windows, and macOS. Flatpak and
Snap Steam are also detected on Linux. Installation locations are:

| Platform | Executable location |
| --- | --- |
| Linux / Steam Deck | `$XDG_DATA_HOME/tempest2032/tempest2032`, normally `~/.local/share/tempest2032/tempest2032` |
| Windows | `%LOCALAPPDATA%\Tempest 2032\tempest2032.exe` |
| macOS | `~/Library/Application Support/Tempest 2032/tempest2032` |
| Flatpak / Snap Steam | `tempest2032/tempest2032` inside that Steam installation's data directory |

All game assets and levels are embedded, so the installed executable runs
independently of the source tree or downloaded copy. Re-running the command
updates that installed copy while retaining its Steam shortcut ID, custom name,
launch options, working directory, icon, tags, and other existing settings.
The installer also adds the simplified favicon as the shortcut icon and the
square spectrum portal as Steam library artwork: landscape and portrait covers,
a hero background, and a transparent logo. Images are embedded in the executable
and installed into the selected account's `userdata/<account>/config/grid/` directory using the
shortcut ID. Existing custom artwork and nonempty shortcut icons are preserved;
re-running the installer fills missing artwork and adds an icon to older shortcuts.
It also replaces the old bundled hero with the reframed version when its bytes
match the original gameplay image.
The hero background is a gameplay screenshot of level 8's long V-shaped web,
with a gold claw, enemies, and explosion rings, without HUD or announcement overlays.
The screenshot fits within a centered 440-pixel-high area of the 1920×620 hero,
leaving room for Big Picture's vertical crop and bottom-left logo.
Running it from the installed copy also works and skips copying onto itself.
Unix executable permissions are preserved.

To regenerate the embedded PNG artwork from the SVGs in `logos/`, run
`uv run tools/steam_artwork.py`. This needs Python, Cairo, and fonts for the
cover captions; `uv` installs the script's Python dependencies. Normal builds and
installation use the bundled PNGs and need no image conversion tools.
The hero is generated from `assets/steam/gameplay.png`; to replace that source,
run `uv run tools/steam_artwork.py --hero-source /path/to/screenshot.png`.
The README uses `assets/gameplay.gif`, a cropped 4.8-second excerpt of scripted
level 8 gameplay at 960×465 and 25 frames per second, showing particle-laser fire
and enemy explosions. Clicking it opens the full-size Steam screenshot.

If detection is ambiguous or Steam is installed in a custom location, use:

```sh
tempest2032 --steam-install --steam-dir "/path/to/Steam" --steam-user 12345678
```

`--steam-user` is the numeric account ID from the `userdata/` directory,
not the full 17-digit Steam ID. Both overrides are optional and require
`--steam-install`. Installation cannot be combined with gameplay or capture
options. If Steam is absent, an account has never signed in, or its configuration
is unreadable, the command exits with an explanatory error.
`--steam-dir` points to the Steam data directory containing `steamapps/` and
`userdata/`. On macOS, this is normally `~/Library/Application Support/Steam`.

Existing shortcuts and unrecognized binary VDF fields are preserved. Before a
changed shortcut file is replaced, its exact contents are saved beside it as
`shortcuts.vdf.tempest2032-backup`, with numbered suffixes if needed. File replacements
are atomic, and a `shortcuts.vdf.tempest2032.lock` file prevents concurrent Tempest 2032 installers.
This is local non-Steam-game registration; it does not publish a Steam store app.

## Sound effects

Original Tempest 2000 samples play for shots, explosions, jumps, power-up drops
and collection, spike and armor hits, Flipper landings, Pulsar pulses, deaths,
warp events, and voice announcements. The level-exit engine uses the original
loop regions and changing pitch.

Use `--volume 0.3` for quieter audio or `--volume 0` for silence. The default
is `0.6`; values must be between zero and one. The sound pack is embedded in the
executable, so playback needs no original source tree or ROM at runtime.

Effects, voices, and engine loops use the original driver rates without a
global pitch multiplier.
Gameplay-controlled pitch changes still apply to individual sounds.

Spike hits and laser shots each own one dedicated voice. Following `PLAYFX2`'s
tie-breaking (`moomoo.dat:$4370–$446a`), each new hit or shot replaces the
previous one instead of stacking copies, so both play at the normal effect level
during rapid hits or sustained fire. Other one-shot effects share a separate
sixteen-voice pool so explosions and announcements remain audible during busy
scenes. This is a deliberate difference
from the original, where `START_MOD` leaves only two shared effect voices while
music plays. Each pool takes a free voice or replaces its lowest-priority voice
when the new priority is equal or higher; lower-priority requests are dropped.
Spike hits, laser shots, and other effects cannot interrupt each other. Sounds are not queued,
and playback completion frees a slot immediately.

YES! YES! YES! replaces the exit-engine sounds with the original looping `yes`
sample. It plays its intro once, then repeats the stored loop region continuously
until the exit flash starts, following `yeson` and `zoo2off`
(`yak.s:16332-16338`, `yak.s:13972-13981`, sample-table entry 23).
The flash stops the loop and clears old effects before playing the warp sound;
restarts and interrupted exits also stop it. Its lifetime follows the exit,
independently of the text animation or a fixed repetition count.
Its pitch rises during exit: `czoom1` and `czoom2` call `zoomup`, which adds one
to each YES voice's DSP playback increment per 60 Hz tick
(`yak.s:13919`, `yak.s:13952`, `yak.s:14010-14047`). `CHANGEFX` applies the increment
after `PLAYFX2`'s 28/43 conversion, so the initial step 333 becomes 393 after a
second, increasing playback speed by about 18%. Frozen simulation holds the
pitch steady; standalone message previews use the same 60 Hz rise in virtual time.

To regenerate `assets/sounds.bin` from the original source tree:

```sh
python3 tools/sounds.py /path/to/tempest2000 assets/sounds.bin
```

The same tool accepts the original 2 MiB cartridge as input:

```sh
python3 tools/sounds.py "/path/to/Tempest 2000.j64" assets/sounds.bin
```

The pack retains signed 8-bit PCM, pitch values, and loop metadata from
`src/incbin/sound_samples_table.bin`. The table's legacy "period" field is a
direct playback increment for effects. Playback follows the Jaguar driver in
`src/dat/moomoo.dat`: multiply pitch by 28/43, then advance an 8:8 sample position
at the SSI rate with clock divider 27. Higher pitch values play faster.
Explosion pitch varies without consuming gameplay random numbers.
Spike hits play sample 13, "Tink For Spike", with a playback increment of
`524 + (remaining_integer_length & 511)`, sampled before removing three units,
following `decspike` (`yak.s:11931–11943`, cartridge file offset `0xb996`).
The embedded 1,038-byte sample matches the cartridge exactly. Spike playback
reproduces the Jaguar's fixed SSI output stage at about 14,838.7 Hz: advance an
8:8 sample position by `floor(pitch * 28 / 43)` each tick, then read the indexed
signed byte. The mixer is embedded in `moomoo.dat` at `$5ed4`, copied to
`$f1b000`. Its four music voices interpolate, but effect voices 4 and 5 use
uninterpolated samples (`$f1b1a4–$f1b21c`, `$f1b244–$f1b24c`, and
`$f1b2a6–$f1b2b2`).

This output stage matters because the spike tone exceeds the SSI Nyquist
frequency. Its alias is the original game's lower, rising sound: trimming a
30-unit spike to zero moves its main peak from about 4.67 kHz to 5.20 kHz, even
though the sample increment falls from 360 to 341. Sending the source PCM
straight to a modern output at its calculated playback rate instead produces
a roughly 10 kHz tone with the wrong pitch direction. Earlier inverted or
offset length curves tried to compensate for that missing output stage and
have been removed. A spectral regression checks the actual decoded waveform.
The upgraded laser fires every three ticks instead of four. Its long sample is
more than twice as loud as the plain shot and overlaps itself during sustained fire.
It plays at a fifth of the usual effect gain to keep sustained fire in balance
with the normal shot.
The converter limits the alternate normal-shot sample
to its physical boundary and trims the first "sexy yes" sample to its declared
length. The ROM omits two unused death samples; a ROM-generated pack leaves
those entries empty, without affecting gameplay sounds.

## Music

The seven original MOD tracks are embedded and decoded in stereo during
playback. Music plays at normal tracker pitch and tempo.
Use `--music-volume 0.5` to lower the music relative to effects
or `--music-volume 0` to disable music. The default is `1.0`, multiplied by the
master `--volume` setting.

Track selection follows `yak.s` and `images_sounds.s`:

| Levels or scene | Original module |
| --- | --- |
| 1-16 and 65-80 | `rave4.mod` |
| 17-32 and 81-96 | `tune7.mod` |
| 33-48 and 97-99 | `tune5.mod` |
| 49-64 | `tune12.mod` |
| Gate flight and psychedelic rings | `tune1.mod` |
| Green-track tunnel | `tune3.mod` |
| Game over | `tune13.mod` |

Tracks loop and continue across levels or retries that use the same tune.
Changing tracks fades the previous music down over one second before starting
the next. Music stops with the application; it does not require FFmpeg or an
external tracker player.

Welcome, Configuration, and initials entry use `tune13.mod`. The original
`rreset` title-screen setup requests `modnum = 1` at `yak.s:1018`; entry one in
`images_sounds.s:63-70` is `tune13.mod`, also used by game over.

Native level overrides can set a top-level `soundtrack = "music/song.mp3"` or an
HTTP/HTTPS URL. Relative paths resolve beside the TOML file; absolute paths are
accepted. MP3, FLAC, Ogg Vorbis, WAV, MOD, and XM are detected from their contents.
At startup, each distinct source is loaded and checked with its audio decoder.
Files load immediately. URLs load one after another on a background thread, so a
slow server never delays the first frame; their levels play the original music
until the track arrives, then fade to it.
Tracker modules stream through the existing tracker player with owned module
data, rather than rendering a whole song into memory.

URLs are cached under `dirs::cache_dir()/tempest2032/soundtracks`, using a BLAKE3
hash of the URL without its fragment. Query parameters remain part of the key.
A valid cache entry is used without contacting the server, allowing offline
playback. Missing or broken entries are downloaded with a 30-second timeout and
128 MiB limit. Only decodable downloads are cached, using an atomic temporary-file
replacement. Cache write failures still allow playback for the current session.
Delete a cache entry to refresh a URL whose contents have changed. The
[demo guide](../examples/custom-level/README.md#soundtrack) lists platform paths.

Missing, unreadable, invalid, or unavailable sources log a warning and fall back
to that level's embedded track. Omitting the setting also keeps the original
music. Custom tracks loop, obey pause and volume settings, and use the normal
fade-out on track changes. Levels referencing the same resolved source keep
playback continuous. Menu, game-over, and warp music still use the embedded
modules. Restart to reload changed local files. Browser builds remain
self-contained and do not load custom soundtracks from files or URLs.

Regenerate the embedded modules from the original source tree or cartridge:

```sh
python3 tools/music.py /path/to/tempest2000 assets/music
python3 tools/music.py "/path/to/Tempest 2000.j64" assets/music
```

## Editable levels

The runnable [custom-level demo](../examples/custom-level/) includes a TOML file
and a field-by-field guide to geometry, enemies, colors, power-ups, and music.
Run `cargo run --release -- --levels examples/custom-level --level 1` to try it.

All shipped levels are embedded in the executable. At startup, TOML files in
`levels/`, or the directory selected by `--levels`, take priority over embedded
definitions with the same `number`. Missing levels use the embedded versions,
so the game runs with an absent, empty, or partial levels directory.
Edit an override file and relaunch the app; no Rust changes are needed.
Existing invalid or unreadable files still produce an error, and duplicate
numbers and gaps beyond the bundled catalog still fail validation.
The shipped TOML files are included automatically when compiling, including
newly added levels. You can copy just the executable to another PC; the
`levels/` directory is optional for customization.

### Released-game web ordering

Tempest 2000 uses the 32-entry sequence at `16 + ((level - 1) & 31)` in the
runtime `raw_webs` table (`yak.s:3567-3571`, `yak.s:8605-8613`). The labelled
RAM defaults at `yak.s:23916` are not the released ordering: `Init` copies the
`romstart` defaults block over `copstart` (`yak.s:878-884`). The table inside
that ROM block (`yak.s:23731-23733`) differs from the later RAM initializer.
The cartridge `orig/t2k.jag` confirms the ROM-initialized table at file offset
`0x19cf0`; its entries are pointers into the cartridge's raw web structures.
The geometry/start-lane regression test fingerprints those actual cartridge
profiles for all 100 levels, with level one's documented spawn exception.

The released 32-web sequence, repeated for later color/enemy sets, is:

```text
 1–16: web5, web11, web1, web2, web9, web3, web18, web19,
       web20, web4, web14, web10, web17, web6, web16, web22
17–32: web24, web23, web27, web25, web21, web28, web29, web30,
       web32, web33, web34, web35, web36, web37, web26, web31
```

In particular, level 8 is the long wide V (`web19`), level 9 is the giraffe
neck (`web20`), level 13 is the wobbly U (`web17`), and level 16 is the
twelve-lane star (`web22`). The sine-wave `web13` is present in the classic
table but never selected by the released Tempest 2000 sequence. Selection,
ordinary advancement, and warp destinations all use the same TOML catalog.

### Level details

The supplied levels are:

- `levels/01.toml`: the existing `web5` level, with 25 Flippers, first spawn after
  61 gameplay ticks, and subsequent spawns every 121 ticks.
- `levels/02.toml`: the original `_nw2` on the square `web11`, with 16 lanes,
  25 Flippers, first spawn after 51 ticks, and subsequent spawns every 101 ticks.
  The Flippers climb at the same `0xA000` speed as level one. They fold through
  180 degrees on straight sides and 90 degrees around corners, using the
  original per-lane orientations.
- `levels/03.toml`: the original `_nw3` on the open, eleven-lane flat `web1`.
  Flippers and Flipper Tankers share a 17-enemy generation budget. Tankers
  arrive first after 223 gameplay ticks, then every 445 ticks. Their rotating
  `s_fliptank2` shells use the original crossed faces and cycling colors.
  Shooting one or letting it reach the rim awards 100 points and sends
  two Flippers into neighboring lanes. They finish their initial flip before
  climbing; the original 8-in-256 mutation chance produces fast blue Flippers
  that scatter further before returning to normal pursuit.
- `levels/04.toml`: the open fifteen-lane V, `web2`, with a shared 25-spawn
  budget for Flippers, Tankers, and Spikers. A Spiker uses the original rotating
  spiral outline and creates a 30-unit spike in a free lane, or extends the
  shortest existing spike when every lane is occupied. It makes 51 extensions
  at `0xD000` speed, retreats, and picks another spike. Growth starts when the
  builder passes the tip using `climbspike`'s integer Z-word comparison, which
  can extend before a full fixed-point comparison would. Visible lengths use
  `draw_spike`'s two-unit steps rather than smooth fractional growth.
  Only one Spiker is
  active at a time. Shooting a spike removes three units and scores three
  points; particle shots continue through it. Destroying the final segment
  scores another three points. Spikes remain after their builder dies and do
  not block level completion, but can kill the glider during the exit flight.
  The exit displays "AVOID THE SPIKES" and still permits movement and firing
  until the rear. A spike collision pulls the camera back before losing a life.
- `levels/05.toml`: the eighteen-lane triangle `web9`, with a 25-spawn budget,
  `0xA400` Flippers, `0x8800` Tankers, and `0xE000` Spikers. Tankers repeat
  every 281 ticks. Pickups drop on the first kill and then every sixth kill,
  matching the next `pup_stuff` group.
- `levels/06.toml`: the open fifteen-lane U-shaped `web3`, with a 31-spawn
  budget. Flippers repeat every 96 ticks and Tankers every 261 ticks, retaining
  level five's climbing speeds. Spikers repeat every 801 ticks.
- `levels/07.toml`: Tempest 2000's fourteen-lane lemon-shaped `web18`, with a
  31-spawn budget, `0xA600` Flippers, and `0xF000` Spikers. The Tempest 2000 mode adds
  16 to the web-table index, so this is `web18`, not classic mode's `web12`.
  Flippers repeat every 91 ticks, Tankers every 251, and Spikers every 751.
- `levels/08.toml`: the open fourteen-lane long wide V `web19` and 31-spawn budget, with Tankers every
  241 ticks and Spikers every 651 ticks. Its Tanker-child mutation threshold
  rises to 28 out of 256.
- `levels/09.toml`: the open thirteen-lane giraffe-neck `web20`, with a 31-spawn
  budget, `0xA800` Flippers, and `0x10000` Spikers. Two alien shots can now be
  active at once, following the original `ashots` table calculation. Tankers
  repeat every 231 ticks and pickups every seventh kill after the first.
- `levels/10.toml`: the closed clover `web4`, introducing Beasties every 2,001
  ticks. Their original `hornm1` core and two `hornm2`/`hornm3` armor layers are
  drawn together, with the core scaled to meet the horn roots. Rail hits strip
  horns before killing the core; stripped
  horns fly back at half shot speed, occupy their original player shot slots,
  and absorb ordinary shots. Beasties pursue without the normal rim pause.
- `levels/11.toml`: the open parabolic `web14`, introducing Fuseballs. Their
  five independently shaped colored legs use `fbpiece1`/`fbpiece2` and the
  original five hue values. They climb along lane boundaries for 49 ticks,
  then cross in sixteen steps with a five-tick delay per step. Only the middle
  of a crossing is shootable. A crossing at the rim can fry the glider even
  during active firing, using the original "FRIED YOU!" death sequence.
  Kills award the original random 250, 500, or 750 points without a pickup drop.
- `levels/12.toml`: the closed cross `web10`, including the original inward
  corner orientations. Flippers repeat every 81 ticks and Fuseballs every 701.
- `levels/13.toml`: Tempest 2000's open wobbly-U `web17`, with twelve lanes and
  `0xAC00` Flippers. This mode-specific selection differs from classic `web15`.
  Fuseballs climb at `0x8800` and repeat every 601 ticks.
- `levels/14.toml`: the open distorted-W `web6`, with fifteen lanes and
  Flippers every 76 ticks. The local flat sections retain their source angles.
- `levels/15.toml`: open `web16`, with `0xAE00` Flippers and `0x13000` Spikers.
  Fuseballs climb at `0x9000` and repeat every 501 ticks.
- `levels/16.toml`: Tempest 2000's twelve-lane star `web22`, with a 41-spawn budget,
  Flippers every 71 ticks, and the original 60-in-256 fast-child threshold.
- `levels/17.toml`: the eighteen-lane big oval `web24`, starting Tempest 2000's second
  web set with red wireframes and cyan Flippers. The 25-spawn wave includes
  Pulsars every 601 ticks, Spikers every 751, Beasties every 1,001, and
  Fuseballs every 901. There are no Flipper Tankers, and three enemy shots
  can be active at once. Flippers return to the `0xA800` speed group.
  Pulsars climb at `0x8000` and share a sixteen-phase cycle, advancing every
  nine ticks. Phase seven electrifies the occupied lane, frying a grounded
  glider even while it fires. Jumping and respawn invulnerability protect it.
  Pulsars wait for a safe phase before entering the web, then split into two
  rim sparks that move in opposite directions every five ticks, wrapping on
  closed webs and bouncing at open ends. Rail kills award 200 points and
  spark kills 150; both participate in the normal pickup-drop sequence.
- `levels/18.toml`: the open fifteen-lane spiraloid `web23`, with a 31-spawn
  budget. Flippers repeat every 101 ticks and Pulsars every 501. It retains
  level seventeen's red wireframes, cyan Flippers, climbing speeds, and other
  generators. Pulsar sparks bounce at the open ends.
- `levels/19.toml`: the closed fifteen-lane pentagon `web27`, with a 31-spawn
  budget. Flipper Tankers return every 445 ticks with a 72-in-256 fast-child
  threshold. Flippers climb at `0xAA00`, Fuseballs at `0x9800`, and Pulsars
  repeat every 601 ticks.
- `levels/20.toml`: the open twelve-lane hook `web25`, with a 33-spawn budget.
  Flippers repeat every 81 ticks, Tankers every 334, and Pulsars every 501.
  The Tanker-child mutation threshold rises to 76 out of 256. Climbing speeds
  stay in level nineteen's group, and Pulsar sparks bounce at the open ends.
- `levels/21.toml`: compact eight-lane `web21`, with 33 shared spawns,
  `0xAC00` Flippers, and Pulsars every 433 ticks. Their phase delay drops
  to eight ticks. Tankers repeat every 281 ticks with an 80-in-256 mutation
  threshold; Spikers climb at `0xE000` and Fuseballs at `0xA000`.
- `levels/22.toml`: open twelve-lane inverted-bird `web28`, with 36 shared
  spawns. Flippers repeat every 96 ticks, Tankers every 261, and Pulsars every
  426. Pulsar sparks bounce at the wing tips.
- `levels/23.toml`: closed fifteen-lane D-shaped `web29`, with six flat lanes
  along its base. Flippers climb at `0xAE00`, Spikers
  at `0xF000`, and Fuseballs at `0xA800`; Pulsars repeat every 463 ticks.
- `levels/24.toml`: open thirteen-lane curved-L `web30`.
  Tankers repeat every 241 ticks, Spikers every 651, and
  Pulsars every 412, with the same 36-spawn budget as levels 22 and 23.
- `levels/25.toml`: open twelve-lane diagonal double-curve `web32`, with 41 shared spawns
  and Fuseball Tankers every 2,001 ticks. Their rotating five-armed graphic
  draws both original leg shapes per arm. Shooting a Tanker or letting it
  reach the rim awards 100 points and releases two Fuseballs at its depth,
  crossing outward from the lane's two edges. Children do not consume the
  generation budget, and opening the Tanker does not advance the pickup counter.
  Pulsar phases now last seven ticks and pickups drop every eighth kill.
- `levels/26.toml`: closed sixteen-lane cat-head `web33`, including its
  inward ear corners. Pulsars repeat every 400 ticks and Spikers every 851; Fuseball
  Tankers retain their 2,001-tick interval.
- `levels/27.toml`: closed fifteen-lane teardrop `web34`, with its retraced tip and
  `0xB200` Flippers, `0x11000` Spikers, and `0xB800` Fuseballs. Pulsars repeat
  every 389 ticks and Tankers every 371.
- `levels/28.toml`: open fourteen-lane crinkly-L `web35`, with alternating
  stem angles. Flippers repeat every 81 ticks, Pulsars every 378, and
  Fuseballs every 701. The fast-child threshold rises to 108 out of 256.
- `levels/29.toml`: closed thirteen-lane kiss-of-death `web36`, retaining its
  shallow inward corners. Flippers climb at `0xB400`, Tankers at `0x9800`, Spikers
  at `0x12000`, and Fuseballs at `0xC000`. Pulsar phases drop to six ticks.
- `levels/30.toml`: open fifteen-lane backwards-C `web37`.
  Flippers repeat every 76 ticks, Pulsars every 351,
  and Fuseball Tankers every 1,001, twice their previous frequency.
- `levels/31.toml`: closed sixteen-lane tiny-star `web26`, with its retraced
  tips and sharp lane reversals. Flippers climb at `0xB600`, Spikers at
  `0x13000`, and Fuseballs at `0xC800`; Flippers repeat every 96 ticks.
- `levels/32.toml`: open seventeen-lane mouse-pointer `web31`, finishing the
  second web set. Fuseballs and Fuseball Tankers both repeat every 501 ticks,
  while Pulsars repeat every 901. Four enemy shots can be active at once.
- `levels/33.toml`: returns to sixteen-lane `web5`, now with yellow wireframes
  and yellow-green Flippers. The 25-spawn wave introduces Pulsar Tankers
  every 501 ticks and Beasties every 701. A Pulsar Tanker draws two opposing,
  rotating zigzags synchronized with the shared pulse. Opening it awards 100
  points and releases two children that flip into adjacent lanes before
  climbing at `0x8000` and electrifying them. There are no direct Pulsar,
  Flipper Tanker, or Fuseball Tanker generators in this wave. Pickups now
  drop every ninth kill after the first.
- `levels/34.toml`: square `web11`, with 31 shared spawns and Pulsar Tankers
  every 451 ticks. Beasties retain their 701-tick interval.
- `levels/35.toml`: open eleven-lane flat `web1`, restoring Flipper Tankers
  every 445 ticks with a 136-in-256 mutation threshold. Flippers climb at
  `0xB200` and Fuseballs at `0xA800`.
- `levels/36.toml`: open fifteen-lane V-shaped `web2`, with 33 shared spawns.
  Flippers repeat every 81 ticks, Flipper Tankers every 334, Pulsar Tankers
  every 401, and Fuseballs every 951.
- `levels/37.toml`: eighteen-lane triangle `web9`, with `0xB400` Flippers,
  `0x9800` Tankers, and `0xB000` Fuseballs. Pulsar phases drop to six ticks;
  Spikers repeat every 801 ticks.
- `levels/38.toml`: open fifteen-lane U-shaped `web3`, with 36 shared spawns.
  Flippers repeat every 96 ticks and Flipper Tankers every 261, while Pulsar
  Tankers retain their 451-tick interval.
- `levels/39.toml`: fourteen-lane Tempest 2000 lemon `web18`, with `0xB600` Flippers,
  `0xF000` Spikers, and `0xB800` Fuseballs. Pulsar Tankers repeat every 401
  ticks and Flipper Tankers every 251.
- `levels/40.toml`: open fourteen-lane long wide V `web19`, with Pulsar Tankers every 351 ticks,
  Flipper Tankers every 241, and Spikers every 651. The Flipper Tanker
  mutation threshold rises to 156 out of 256.
- `levels/41.toml`: open thirteen-lane giraffe-neck `web20`, with 41 shared
  spawns and all three Tanker types. Pulsar Tankers repeat every 351 ticks,
  Fuseball Tankers every 801, and direct Pulsars every 1,101. The shared
  Pulsar phase delay drops to five ticks.
- `levels/42.toml`: sixteen-lane clover `web4`, retaining its inward corners.
  Beasties repeat every 701 ticks, Spikers every 851, and Fuseball Tankers
  every 2,001. Direct Pulsars still repeat every 1,101 ticks.
- `levels/43.toml`: open thirteen-lane parabolic `web14`, with `0xBA00`
  Flippers and `0xC800` Fuseballs. Fuseball Tankers repeat every 301 ticks,
  Fuseballs every 401, and Pulsar Tankers every 501.
- `levels/44.toml`: sixteen-lane cross `web10`. Direct Pulsars and Pulsar
  Tankers both repeat every 501 ticks; Fuseball Tankers remain on their
  301-tick interval, and Flippers repeat every 81 ticks.
- `levels/45.toml`: open twelve-lane wobbly-U `web17`, with
  `0xBC00` Flippers and `0xB800` Tankers. Pulsar phases drop to four ticks;
  Pulsar Tankers repeat every 301 ticks and direct Pulsars every 1,001.
- `levels/46.toml`: open fifteen-lane distorted-W `web6`, with Flippers every
  76 ticks and a 180-in-256 fast-child threshold. Other generator intervals
  and speeds remain in level forty-five's group.
- `levels/47.toml`: open fifteen-lane `web16`, with `0xBE00` Flippers,
  `0x13000` Spikers, and `0xD800` Fuseballs. Direct Pulsars now repeat every
  351 ticks while Pulsar Tankers remain at 301.
- `levels/48.toml`: twelve-lane star `web22`, finishing the third web set.
  Flipper Tankers repeat every 231 ticks, Pulsar Tankers every 301, and
  Fuseball Tankers every 401; the fast-child threshold reaches 188 out of 256.
- `levels/49.toml`: eighteen-lane big oval `web24`, starting the fourth web
  set with unfilled webs and pale-cyan Flipper colors. This wave consists
  entirely of 17 armored Beasties: first spawn after 71 gameplay ticks,
  then every 141 ticks, climbing at `0xB000`. Normal Flipper generation is
  disabled, and there are no other enemy generators.
- `levels/50.toml`: open fifteen-lane spiraloid `web23`, restoring normal
  Flippers every 51 ticks in a 41-spawn mixed wave. Beasties repeat every
  501 ticks, Spikers every 751, and Fuseballs every 901.
- `levels/51.toml`: closed fifteen-lane pentagon `web27`, with 46 shared
  spawns. Pulsar Tankers repeat every 951 ticks in place of Fuseballs;
  Flippers climb at `0xB200`, and Beasties repeat every 501 ticks.
- `levels/52.toml`: open twelve-lane hook `web25`, increasing the shared
  budget to 51 spawns. Beasties repeat every 301 ticks, Fuseballs every 951,
  and normal Flippers continue every 51 ticks. Pulsar Tankers are absent.
- `levels/53.toml`: compact eight-lane `web21`, restoring Flipper Tankers
  every 281 ticks with a 208-in-256 fast-child threshold. Beasties repeat
  every 201 ticks; Flippers climb at `0xB400` and Spikers at `0xE000`.
- `levels/54.toml`: open twelve-lane inverted-bird `web28`, with Beasties
  every 101 ticks and Flipper Tankers every 261. The shared budget stays
  at 51 spawns, with a 212-in-256 fast-child threshold.
- `levels/55.toml`: closed fifteen-lane D-shaped `web29`, preserving its
  flat base. Flippers and Beasties climb at `0xB600`, Spikers at `0xF000`,
  and Fuseballs at `0xB800`; Flipper Tankers repeat every 251 ticks.
- `levels/56.toml`: open thirteen-lane curved-L `web30`, with Flippers every
  51 ticks, Beasties every 101, Flipper Tankers every 241, and Spikers every
  651. The fast-child threshold reaches 220 out of 256.
- `levels/57.toml`: open diagonal double-curve `web32`, with 61 shared spawns. Dedicated
  Super Flippers appear first after 13 ticks, then every 26, using the blue
  `supf1` mesh and double-speed rim flips without a normal pause. Mirrors
  repeat every 401 ticks, spin, and stop forty units from the rim. Surviving
  hits push a Mirror backward by sixteen climbing steps and return the shot
  at full speed, retaining its magazine slot. Five hits or recoil beyond
  the rear destroys it for 250, 500, or 750 points without a pickup drop.
- `levels/58.toml`: closed sixteen-lane cat-head `web33`, with 41 shared
  generation attempts. The executable wave table selects Spikers every 51
  ticks, despite its "Flippers" comment. Only one Spiker can be active;
  blocked attempts still count against the budget, as in `launch1`.
  Mirrors repeat every 401 ticks and Pulsars every 152, with five-tick phases.
- `levels/59.toml`: closed fifteen-lane teardrop `web34`, adding
  Fuseballs every 524 ticks. Mirrors and Fuseballs use the `0xC800` climbing
  speed group, while the primary Spiker attempts retain their 51-tick interval.
- `levels/60.toml`: open crinkly-L `web35`, with 51 shared attempts and
  a second Spiker generator every 1,501 ticks. Both Spiker timers share the
  original one-active-Spiker limit; blocked attempts still consume budget.
  Mirrors, Pulsars, and Fuseballs keep level fifty-nine's intervals.
- `levels/61.toml`: closed kiss-of-death `web36`, retaining its inward corners.
  Mirrors and Fuseballs climb at `0xD000`, Spikers at `0x12000`, and Pulsar
  phases drop to four ticks. Generator intervals remain those of level 60.
- `levels/62.toml`: open fifteen-lane backwards-C `web37`, replacing direct
  Pulsars with Pulsar Tankers every 352 ticks. Both Spiker timers and the
  Mirror and Fuseball generators retain their previous intervals.
- `levels/63.toml`: closed sixteen-lane tiny-star `web26`, with Pulsar
  Tankers every 252 ticks. Mirrors and Fuseballs climb at `0xD800`, while
  Spikers use the final fourth-set `0x13000` speed group.
- `levels/64.toml`: open seventeen-lane mouse-pointer `web31`, ending the fourth
  web set with primary Pulsars every 31 ticks and Beasties every 401. Normal
  Flippers, Mirrors, Spikers, and Tankers are absent from this 51-spawn wave.
- `levels/65.toml`: returns to sixteen-lane `web5`, starting the fifth set
  with magenta wireframes and pale-cream Flippers. The dispatch table selects
  Super Flippers every 21 ticks and Alien Droids every 701. Alien Droids
  climb at twice the Flipper speed to fifteen units beyond the rim, cross
  a lane in sixteen three-tick steps, then zap for 32 ticks. Jumping and
  invulnerability protect the player. Shots repel an ascending Droid by
  sixteen climbing steps; recoil beyond the rear or a hit in its rim phase
  destroys it for bonus points without a pickup drop.
- `levels/66.toml`: square `web11`, with Super Flippers every 41 ticks,
  Mirrors every 751, and Alien Droids every 601. The source's primary
  dispatch remains type 7, despite its "Flippers3" comment.
- `levels/67.toml`: open eleven-lane flat `web1`, adding Fuseballs every 501
  ticks. Alien Droids climb at `0x16400`, twice the `0xB200` Super Flipper
  speed; Mirrors and Fuseballs use `0xA800`.
- `levels/68.toml`: open fifteen-lane V-shaped `web2`, preserving level 67's
  Super Flipper, Mirror, Alien Droid, and Fuseball intervals in a 61-spawn wave.
- `levels/69.toml`: eighteen-lane triangle `web9`, replacing Fuseballs with
  Pulsar Tankers every 501 ticks. Super Flippers climb at `0xB400`, Alien
  Droids at `0x16800`, and the shared Pulsar phase delay is six ticks.
- `levels/70.toml`: open fifteen-lane U-shaped `web3`, with primary Pulsar
  Tankers every 71 ticks instead of Super Flippers. Fuseballs return every
  301 ticks, while Mirrors and Alien Droids retain their previous intervals.
- `levels/71.toml`: fourteen-lane Tempest 2000 lemon `web18`, replacing Fuseballs with
  Beasties every 501 ticks. Primary Pulsar Tankers remain on their 71-tick
  interval, alongside Mirrors and faster `0x16C00` Alien Droids.
- `levels/72.toml`: open long wide V `web19`, restoring Super Flippers every 21
  ticks and adding Spikers every 501. The 61-spawn wave retains Mirrors and
  Alien Droids, while Pulsar Tankers and Beasties are absent.
- `levels/73.toml`: open thirteen-lane giraffe-neck `web20`, adding Fuseballs
  every 301 ticks to the Super Flipper, Mirror, Alien Droid, and Spiker wave.
  Alien Droids climb at `0x17000` and Spikers at `0x16000`.
- `levels/74.toml`: closed clover `web4`, adding type-3 Super Flippers every
  401 ticks. They use the six-face `s_flip2` mesh, flip away from the player
  while descending, stop descending when aligned, and pursue normally at
  the rim. Unlike type 2, they use the normal four-unit angular step. The
  source's extra firing call is commented out and remains inactive here.
- `levels/75.toml`: open parabolic `web14`, restoring Flipper Tankers every
  701 ticks. The fast-child threshold is capped at 252 out of 256; an
  independent 148-in-256 check can turn a mutated child into type 3.
  Spiral children descend during their release flip and then flee the player.
- `levels/76.toml`: closed cross `web10`, adding direct Pulsars every 301
  ticks with five-tick phases to the full type-2/type-3, Mirror, Alien Droid,
  Spiker, Fuseball, and Flipper Tanker wave.
- `levels/77.toml`: open wobbly-U `web17`, introducing Fuseball Tankers
  every 624 ticks in place of direct Pulsars. Alien Droids climb at `0x17800`,
  while Mirrors and Fuseballs use the `0xD000` group.
- `levels/78.toml`: open distorted-W `web6`, adding direct Pulsars every
  301 ticks alongside Fuseball Tankers. Both share the existing 61-spawn
  budget, with four-tick Pulsar phases.
- `levels/79.toml`: open fifteen-lane `web16`, adding Beasties every 801
  ticks. Type-2 and type-3 Flippers use `0xBE00`; Alien Droids use `0x17C00`.
- `levels/80.toml`: twelve-lane star `web22`, ending the fifth set with
  both Super Flipper types, all three Tanker types, Mirrors, Alien Droids,
  Fuseballs, Pulsars, and Beasties. Pulsar Tankers replace Spikers at 501 ticks.
- `levels/81.toml`: big oval `web24`, beginning the sixth set with orange
  wireframes, amber Flippers, and pickups every eleventh kill. It retains the
  full level-80 enemy mix with `0xC000` Flippers and seven-tick Pulsar phases.
- `levels/82.toml`: open spiraloid `web23`, shortening Alien Droid and
  Flipper Tanker intervals to 501 ticks while retaining the full enemy mix.
- `levels/83.toml`: closed pentagon `web27`, with `0xC200` Flippers,
  `0x18400` Alien Droids, and `0xC800` Mirrors and Fuseballs.
- `levels/84.toml`: open hook `web25`, shortening Fuseball Tanker generation
  to 524 ticks while retaining the 61-spawn budget and level-83 speeds.
- `levels/85.toml`: compact `web21`, increasing the budget to 71 spawns.
  Flippers use `0xC400`, Alien Droids `0x18800`, and Pulsar phases drop to six ticks.
- `levels/86.toml`: open inverted-bird `web28`, shortening Alien Droid
  generation to 401 ticks within the 71-spawn wave.
- `levels/87.toml`: D-shaped `web29`, shortening Pulsar Tankers to 451 ticks
  and moving into the `0xC600` Flipper and `0xD800` Fuseball speed groups.
- `levels/88.toml`: curved-L `web30`, retaining the 71-spawn full enemy mix
  and level-87 speed and interval settings.
- `levels/89.toml`: diagonal double-curve `web32`, shortening Flipper Tankers to 301 ticks.
  Flippers use `0xC800`, Fuseballs `0xE000`, and Pulsar phases drop to five ticks.
- `levels/90.toml`: cat-head `web33`, increasing the full enemy-mix
  budget to 81 spawns while retaining level-89 speeds and intervals.
- `levels/91.toml`: teardrop `web34`, shortening type-3 Flippers
  to 351 ticks and moving into `0xCA00`/`0xE800` movement groups.
- `levels/92.toml`: open crinkly-L `web35`, shortening Flipper Tankers to 251 ticks
  within the 81-spawn wave.
- `levels/93.toml`: kiss-of-death `web36`, increasing direct Pulsars to every
  101 ticks with four-tick phases and `0xCC00` Flipper movement.
- `levels/94.toml`: open backwards-C `web37`, retaining the rapid Pulsar and
  dense Tanker settings of level 93 in an 81-spawn wave.
- `levels/95.toml`: tiny-star `web26`, shortening Alien Droids to 201
  ticks and using the final sixth-set `0xCE00`/`0xF800` speed groups.
- `levels/96.toml`: mouse-pointer `web31`, ending the sixth set with 91 spawns
  and Mirrors every 351 ticks alongside the rapid Alien Droid and Pulsar generators.
- `levels/97.toml`: ring `web5`, starting the final cream-wireframe/magenta-
  Flipper group. Pickups now repeat every sixteenth kill, Pulsar phases reset
  to seven ticks, and Fuseball Tankers repeat every 424 ticks.
- `levels/98.toml`: square `web11`, shortening Fuseball Tankers to 324 ticks
  in the 91-spawn cream-palette wave.
- `levels/99.toml`: open flat `web1`, with 101 shared spawns consisting only
  of type-2 Super Flippers every 21 ticks and Beasties every 101. The original
  `_nw99` label aliases `_nw100`; its commented-out alternate wave is unused.
- `levels/100.toml`: open V-shaped `web2`, completing the original 100-level
  catalog with the same 101-spawn Super Flipper/Beastie wave as level 99.
  The configured cycle wraps back to level one after this level.

Clearing a level advances after the exit animation. The last level loops back to
level one, carrying score, lives, and unspent warp tokens. Acquired upgrades
and the pickup sequence reset on each web entry. A death retries the current
level and removes upgrades. Restart in the pause menu starts a fresh run from level one.

Start directly on the second level with:

```sh
cargo run -- --level 2
```

Use `--levels path/to/levels` to load another directory. These options can be
combined with screenshot and recording options, for example:
`cargo run -- --level 2 --record-exit target/square-exit`.

The TOML fields use these conventions:

- `number`: levels are loaded in numerical order, numbered consecutively from 1.
- `web.closed`: defaults to true. Open webs have one more vertex than lanes;
  movement stops at their endpoints and enemies cannot wrap across them.
- `web.vertices`: vertices in the prototype's screen order, on the original
  coordinate grid. Closed outlines are clockwise; do not repeat their first
  vertex. Open outlines list both endpoints and can be a straight line.
- `web.orientations`: one Jaguar angle per lane. A full turn is 256 units;
  negative values turn clockwise. These also determine Flipper flip targets.
- `web.start_lane`: a zero-based index in this clockwise order. Original
  `web11` lane 5 is lane 10 here. Level one's existing prototype spawn is kept.
- `web.depth`: depth in game units, 160 for the supplied webs.
- `flippers`: count, first and repeat spawn times in 60 Hz ticks, climbing speed
  in depth units per tick, flip speed in angle units per tick, pause counter,
  and maximum simultaneous Flippers. Climbing speed converts to 16:16 once
  when the file is loaded.
  `flippers.enabled` defaults to true; set it to false to disable normal
  Flipper generation. The table still supplies shared capacity and rim-motion
  settings, including those used by Beasties and Tanker children. A disabled
  Flipper generator requires another enemy generator in the level.
  Optional `super_flippers` and `mirrors` tables use the standard generator
  fields. Super Flippers have independent spawn timers and double-speed flips.
  Optional `spiral_flippers` tables configure the independent type-3 generator.
  Optional `aliens` tables specify Alien Droid generation and climbing speed.
- `enemy_count`: required generation budget shared by every generator. Released Tanker contents do not
  consume this budget. Optional `tankers` tables specify spawn times, climbing
  speed, and the fast-child probability out of 256.
  `tankers.spiral_chance` defaults to zero and sets the second mutation check
  for a child that already passed `super_chance`.
  Optional `spikers` tables specify their spawn times and climbing speed.
  Optional `extra_spikers` tables add a second timer sharing the one-Spiker
  limit and require a `spikers` table with the same climbing speed.
  Optional `fuse_tankers` tables use the same generator fields and require
  a `fuseballs` table to supply their children's climbing speed.
  Optional `pulsars` tables also specify `phase_ticks` for the shared pulse
  and `propagation_ticks` for rim-spark movement. Both must be positive.
  Optional `pulsar_tankers` tables use the same fields, with `climb_speed`
  controlling the Tanker. Released children use the `pulsars` climbing speed
  when present, or the original `0x8000` otherwise. Both tables must agree
  on phase and propagation timing.
- `appearance.surface` and `appearance.lines`: RGB channels between 0 and 1.
  `surface` is the fallback fill for custom levels without `surface_hues`.
- `appearance.surface_hues`: optional array of Jaguar CRY hue bytes, one per
  lane in the TOML vertex order. Hexadecimal values such as `0x33` are supported.
  This overrides `surface`; an explicit empty array disables the fill entirely.
  Every bundled level specifies its source-derived palette, including empty
  palettes for levels 49–64.
- `appearance.flipper` uses integer RGB channels between 0 and 255.
- `powerups.kill_interval`: first kill drops a pickup, then every Nth kill.
  Each new level starts a fresh drop counter while retaining collected rewards.

Invalid geometry, out-of-range values, numbering errors, and unknown fields
are rejected with the offending file's name. Shared player, weapon, palette,
and animation behavior stays in Rust.

### Web fill colors

The wireframe's `webcol` is not the solid fill color. The `solweb` path sends
the raw web to `donky.gas` (`yak.s:16518-16555`), whose `web3d` reads a hue word
for each lane after the web's orientation table (`donky.gas:341-404`). Thus
level 5's `web9` has blue outlines but blue-to-purple lane fills, from its
`0x04` through `0x71` palette (`yak.s:21020-21025`). Repeated web shapes retain
their fill palette even when later level sets change the wireframe hue.
The `webcol == 1` set, levels 49–64, skips the solid draw
(`yak.s:16522-16525`).

The 100 TOML palettes follow the actual source vertex sequences, reversing and
rotating the hue lists along with the lanes. Full sequences distinguish
retraced edges such as `web26`'s tips. A short or missing source color table is
read through into the following words, just as the GPU does; `web35` retains
those resulting colors rather than inventing a missing palette. Palettes for
the released ordering were also checked directly against the cartridge words.

Shading uses the original sine table, a settled glint of 248, and a rear-rim
intensity one quarter of the front. The shader's phase load is overwritten by
the previous face's hue (`donky.gas:399-419`), so those hues also determine the
relative brightness phases. The first face uses the last face's hue to keep
the cyclic phase deterministic. Vertex colors advance at 60 Hz while geometry
and mesh handles stay fixed. The existing entry/death/exit fade still controls
surface visibility. Custom levels may omit `surface_hues` to retain the RGB
fill and its static depth shading.

### Difficulty

`--difficulty easy|normal|hard` (the former `low` and `high` remain accepted)
selects how fast and how crowded the enemies are. The welcome screen's
configuration menu selects the same settings. The saved menu selection is used on
subsequent launches; an explicit `--difficulty` overrides it for that launch
without changing the saved choice. `normal` is the original game, tick for tick.

Difficulty changes threat only. Every setting keeps the same enemy budget,
power-up drops and reward order, points, extra lives, warp tokens, superzapper,
and starting lives, and the player's movement, firing, and bullets never change,
so skill and progression carry over between settings.

| Setting | Enemy and warp tempo | Simultaneous enemy shots | Enemies at once |
| --- | --- | --- | --- |
| `easy` | ×0.8 | one fewer, at least one | `max_active` − 2 |
| `normal` | ×1 | the level's `ashots` budget | `max_active` |
| `hard` | ×1.2 | one more, at most Beastly's eight | `max_active` + 2 |

```sh
cargo run --release -- --level 3 --difficulty hard
```

The tempo is an enemy clock counting fifths of a 60 Hz tick (`difficulty.rs`).
Each simulation tick advances it by four, five, or six fifths. Movement uses the
clock's exact share of each per-tick step, so climbing, approaching, flipping,
enemy and reflected shots, and warp travel stay smooth. Countdowns run once per
whole enemy tick: Flipper pauses, spawn timers and first spawns, the shared
112-tick fire timer, Pulsar pulse phases and sparks, Fuseball rail waits and
crossings, Alien crossings and zaps, and Spike builds. Every timing therefore
keeps its original relationship to the others: a Pulsar electrifies as often per
climb, an enemy fires as often per climb, and a Spike reaches the same length.
Knock-back still uses the unscaled climb speed, so Aliens and Mirrors take the
same number of hits to destroy on every setting.

A headless balance check plays waves with a scripted, invulnerable player that
steers to the nearest enemy and fires once it reaches the front half of the web.
`cargo test --release balance_report -- --ignored --nocapture` prints, for each
level and setting, the time to clear, enemy shots per minute, the most enemies
at once, and the power-up drops, followed by catalog totals. A regular test
checks levels 1, 9, and 33. Over the waves the script clears, Easy takes about
a third longer with 30% less enemy fire and Hard about a fifth less time with
30% more fire, while total drops stay within about 1%. Drops can still differ by
one or two in a level, because intercepted enemy shots count toward the next
drop, as in the original, and the settings fire different numbers of shots.

Warp zones use the same tempo. Gate flight and the psychedelic rings travel
forward at the scaled speed while steering keeps its original rates, so gates
arrive a fifth slower or faster. The tunnel advances whole track segments, with
its roll, on the enemy clock and draws its rings between segments so that the
scroll stays smooth. Warp presentation timers, such as the melts and the victory
screen, keep their original lengths.

### Beastly mode

The original has a second, unrelated difficulty mode that is **earned, not
selected**. Beating the game sets `gb` at `yak.s:8910`; after the game-over
routine returns, the main loop branches into `dobeastly` (`yak.s:1111`), which
shows a congratulations page and sets `bset.b #2,sysflags` at `yak.s:1290`. That
bit persists in cartridge RAM, so from then on the level selection screen draws
"PRESS option FOR BEASTLY MODE!" (`yak.s:1680`) and pressing Option on that
screen sets the runtime `beastly` flag (`yak.s:1817-1826`). Head-to-head is
excluded, and any ordinary button clears it.

It is not a point on the `--difficulty` scale. Beastly weakens the player and
removes Flipper pauses, while `--difficulty` only changes enemy tempo and
crowding:

| Knob | Beastly | `--difficulty hard` |
| --- | --- | --- |
| Player bullet speed | ×0.50 (`$38000` halved) | unchanged |
| Alien shot budget | absolute 7, not a multiplier | one more, at most 8 |
| Enemy tempo | unchanged | ×1.2 for every enemy timing |
| Enemies at once | unchanged | `max_active` + 2 |
| Flipper pause | never pauses | shorter in real time, same in enemy ticks |

Two corrections worth recording, since both are easy to get backwards.
`shotspeed` is read in `player_shot` (`yak.s:10215`), so halving it slows the
**player's** bullets; the enemy never reads it. And the Flipper fire reload
widens from `$0303` to `$0606` (`yak.s:9430`), which the source comment calls
"make firing less frequent"; the reload only happens when the low byte
underflows, so that is a real reduction, but its intent in a harder mode is not
clear.

A faithful port would need the absolute 7-shot budget, the never-pausing
Flippers, and some substitute for the `sysflags` persistence the prototype does
not have. It is left as future work and is deliberately separate from
`--difficulty`.

### Levels past 100

Clearing level 100 returns to level 1. The original `waves` table in `yak.s`
is longer than that: it holds `_nw1` through `_nw100`, then `_nw85` through
`_nw96`, then a null sentinel that resets the level counter. `rsetw` reads it
with `move.l 0(a0,d0.w)` and falls back to `clr cwave` on the null, so twelve
levels past 100 would replay the wave data from levels 85 to 96 before
restarting. That band is not reachable in normal play. `sweb0` ends the game
once the level counter reaches 99, and level selection caps its maximum at 99,
so the tail only guards against running off the table.

Difficulty is not ramped on any repeat in Tempest 2000. `rsetw` clears `d2`
and skips the whole ramp in Tempest 2000 mode, so the only mechanism that adds
difficulty to a repeated level belongs to classic Tempest: it wraps at level 48
through the sixteen-entry `tradmax` table and adds `(level - 48) >> 7` to every
enemy climb speed, which takes roughly 129 levels to reach one depth unit.
Tempest 2000 takes its enemy speeds from fixed tables indexed by level, holds
Flipper random speed constant, and caps automatic fire at three shots. Level
difficulty therefore comes from the hand-authored wave data, not from a formula.

## Level entry

The level begins with the original `setweb` and `rotate_web` sequence: a
wireframe web approaches from 256 depth units away at one unit per tick. Over
256 ticks its XZ rotation makes a full turn, while its YZ rotation advances
from half a turn to a full turn. It rotates around the tunnel's centre, leaving
the camera and stars independent of the web.

Once the web settles, its blue faces fade in by eight intensity steps per tick
to 248. The claw rezzes over 64 ticks using `srezclaw`, `dsclaw2`, and `draw_z`:
the original `claw1` through `claw8` outlines draw at four coloured depths,
with the spacing contracting from 16 depth units to zero. Movement and firing
are available during this effect; the enemy wave starts counting down as soon
as the web settles. The solid claw replaces the outlines when rezzing ends.

Restarting from the pause menu and re-entering after a lost life replay this sequence.

## Level exit

Clearing the wave starts the original `zoom1` and `czoom1` fly-through. The
camera and claw accelerate down the tunnel using `zs_stuff`'s `0x400` fixed-point
step per tick. The blue faces fade away; movement and firing remain available
until the claw reaches the rear.

At the rear, `czoom2` replaces the solid claw with four spinning wireframe
copies. Their depth spacing grows as they fly away. `zoom2` hides the web and
adds further acceleration to the claw. The starfield uses `starfield1`'s four
depth-spaced samples per star, with separation growing by a quarter unit per
tick. Reaching the source's 2,000-unit claw depth triggers the warp flash.

After the flash, the next configured level begins with score, lives, and
unspent warp tokens preserved. Upgrades reset on entry to the next web.
The last level loops back to the first. Restart in the pause menu cancels the exit
and starts a fresh game.

## Antialiasing

All geometry cameras, including both gameplay layers, the 3D overlay, and the
welcome-screen models, use explicit 4× MSAA. The overlay also draws rewards and
the Game Over and warp-completion bitmaps behind its HUD, so no separate 2D
camera allocates another window-sized depth buffer. A single
FXAA pass on the final 3D overlay camera smooths remaining high-contrast edges
after the scene layers are combined. UI text uses Bevy's font antialiasing;
original bitmap feedback and particle effects retain their original resolution.

Automatic antialiasing tests reductions from 4× MSAA with FXAA to FXAA alone,
then to no filtering. After a 1.5-second warmup, twelve consecutive half-second
windows below 57 rendered updates per second establish a baseline. Requiring six
seconds keeps brief background activity, such as an update check, from starting
a trial. All frame
intervals count, including stalls. The next lower setting is applied, allowed
1.5 seconds to settle, and measured for two seconds. The previous setting is then
restored and measured again the same way, so a background load that ends during
the trial makes both measurements fast instead of crediting the reduction. The
reduction is retained only if, compared with both the baseline and the recheck:

- Mean frame time falls by at least 8% and 0.75 ms, without increasing time over
  the 60 Hz frame budget by more than 0.1 ms per frame; or
- Mean excess time over the 16.67 ms budget falls by at least 25% and 0.5 ms per
  frame, without making mean frame time more than 2% worse.

Otherwise the previous quality stays restored. Both outcomes start a 30-second
cooldown before another baseline can be measured. Logs show the baseline,
reduced, and rechecked frame times, excess time, and the keep/restore decision. This prevents
repeated quality loss when reducing AA does not help.

Confirmed reductions are not permanent. After 60 seconds of measured play without
a slow window, the next higher setting is tried again, up to the starting
quality. It is kept if its two-second measurement, after the usual settling time,
stays at or above 57 updates per second; otherwise the reduction returns and the
calm period before the next attempt doubles, up to 16 minutes. A successful
probe resets that delay. Slow windows restart the calm period, while pauses and
scene changes only interrupt the measurement in progress. The 57 FPS threshold tolerates 59.94 Hz displays and
small scheduling variations around 60 Hz.

Native builds with automatic antialiasing also record per-pass GPU times with
Bevy's `RenderDiagnosticsPlugin`, which Vulkan and DX12 report through timestamp
queries; the log notes when they arrive. With GPU times available:

- Slow windows only count toward a trial when the GPU itself uses more than 75%
  of the 16.67 ms budget. A slowdown with an idle GPU comes from the CPU or
  another process, so antialiasing is left alone.
- A reduction is also required to cut GPU time by at least 10% and 1 ms against
  both the baseline and the recheck, crediting only work it actually removed.
- The calm period before a probe only counts windows whose GPU time is under 60%
  of the budget, and a probe whose frames are slow is still kept when its GPU
  time stays under the 75% bound.

Metal, WebGL2, and WebGPU report no GPU times, so they rely on frame times and
the recheck alone. Fixed `--antialiasing` sessions skip the GPU diagnostics.

Losing focus, hiding the window, resizing, changing levels or game/front-end
phases, and pausing discard measurements. An interrupted trial restores its
previous quality, an interrupted probe restores the reduced quality, and both
start the cooldown; confirmed quality is preserved.
`--antialiasing full`, `--antialiasing fxaa`, and `--antialiasing off` fix the
quality for the entire session, disabling automatic trials and alternate-pipeline
prewarming. The browser accepts the same values as `?aa=`, where `auto` is the
default.

The starting step comes from the host. Browsers report their unmasked WebGL
renderer string and a coarse pointer, which the site reads before the game
starts: a software renderer such as SwiftShader or llvmpipe starts with no
filtering, a touch device starts at FXAA alone, and everything else starts at
4× MSAA. Desktop builds start at 4× MSAA. Browsers reduce the user agent, so
the device model is not identifiable and no device list is used. Measurements
use real time, including in browser builds, and do not alter the fixed 60 Hz
gameplay simulation. Frame times measure application updates, not confirmed
display presentations, and reducing antialiasing cannot fix every cause of slow
rendering.

In automatic mode, the starting step and every step below it are each applied for
one frame during the warmup. The sample count is part of wgpu's pipeline key, so a first-time
step-down otherwise compiles every pipeline of both camera passes in the frame
it lands on, which reads as a freeze; building them during the warmup moves that
cost to startup, while the welcome screen is still loading. The warmup waits
for a focused, visible window, so a browser canvas that starts unfocused still
gets it, and refocusing a backgrounded window does not repeat it.

## HUD

The HUD uses sharp, beveled 3D score numerals with continuous rainbow colour
bands, the actual glider mesh for life icons, and the original shaded warp
pyramids. Digits have front faces, bevels, counters, and shallow extruded sides.
The score's deeper bodies have a larger lower-right extrusion matching the
original's more substantial 3D appearance. Their silhouettes and shading are generated as geometry, with normal display
anti-aliasing and no transparency fringe or low-resolution shading texture.
Up to eight spare-life icons and three warp pyramids are displayed. The active
life is excluded, so the last playable life has no life icon in the HUD.
Life icons and warp pyramids fade and scale in or out over 0.25 seconds when
their counts change. These presentation transitions freeze while paused.
Score and spare lives use the same 16-unit outer margin on the left as the
warp pyramids on the right, scaled together in narrow windows.
New runs start with three spare lives plus the active life, for four lives total.

Scoring grants an extra life at every 20,000-point boundary, including points
from pickups, superzap kills, and warp bonuses. One award can cross several
boundaries and earn several lives. The original `doscore` checks carries into
the ten-thousands digit and calls `do_oneup` when it goes odd to even
(`yak.s:18371-18388`); this is every 20,000 points, not every 100,000.
Each award queues its own 1UP animation and sound, even if a death occurs on the
same tick. The eight visible spare-life icons do not limit the stored life count.

Level, upgrade, control, and bonus-stage debug labels are removed; upgrades
still announce themselves when collected.
Pause shows red-faced, gold-beveled PAUSED lettering in the large font's
proportions over the undimmed frozen scene. `--fps` enables the optional
diagnostic text overlay. The lettering overlay has its own orthographic 3D
camera, excluded from gameplay camera movement and projection systems.

## Controls

| Key | Action |
| --- | --- |
| Left arrow or A | Move clockwise |
| Right arrow or D | Move counterclockwise |
| Z or Space | Fire, hold for repeat fire |
| X | Jump, after collecting the jump upgrade |
| C | Superzapper |
| All four arrows or W/A/S/D | Steer in warp zones |
| P or Pause | Pause or resume |
| Up/down arrows or W/S, then Enter or Space | Select and confirm a pause-menu option |
| Escape | Resume while paused; abandon the run and return to Welcome during gameplay; quit on Welcome |

Controllers work alongside the keyboard through `gilrs`. Buttons are read by
their Xbox position, so a PlayStation pad behaves the same way:

| Controller | Action |
| --- | --- |
| D-pad or left stick | Movement; up and down also fly the warp bonus stage |
| A | Fire, hold for repeat fire |
| B, left trigger, or left bumper | Jump, after collecting the jump upgrade |
| X, right trigger, or right bumper | Superzapper |
| Start | Pause or resume |
| D-pad up/down or left stick up/down, then A | Select and confirm a pause-menu option |

Steam Input should use a Gamepad layout. Steam's virtual gamepad follows this
same mapping; there is no separate bundled Steam keyboard-emulation profile.
Gameplay A/B/X map to Z/X/C, while A/B map to Enter/Escape in front-end screens,
game over, and the pause menu. Held A/B buttons are suppressed until released
when their meaning changes. Enter and numpad Enter only confirm menus; neither
fires. Physical keyboard actions use Z/X/C with Space as an alternate Fire key.

Hold a movement key to accelerate around the rim. The 60 Hz simulation uses
the original 16:16 movement values, acceleration `0x400` per tick and maximum
velocity `0x6000`. That reaches 22.5 lanes per second after 24 ticks.
Movement wraps around closed webs and stops at the endpoints of open webs.
Releasing the movement key stops the claw.
Changing direction stops the claw for one tick before accelerating backwards.
Opposite directions held together cancel each other.

These directions follow the original `claw_right` and `domove`: Right adds to
the original web's counterclockwise lane order. This renderer reverses the
vertex order, so it also reverses the input sign.

### Superzapper

Press `C` during gameplay to trigger the Superzapper. The first use displays
"EAT ELECTRIC DEATH!", plays the original crackle sample, and sweeps eligible
enemies and ordinary enemy shots one target every four simulation ticks.
One press starts the entire sweep. At 60 Hz, consecutive hits are about
67 ms apart; three Flippers disappear within about 0.13 seconds from the
first hit to the last. This cadence was checked frame by frame against a
Jaguar gameplay recording, including its 150, 300, and 450 score updates.
Two vector lightning bolts connect each target to the selected lane's two rim
edges at the claw's current depth. Each has 32 segments, with the original
`ox.gas` 8-bit random sequence adding X/Y displacement in world space before
projection. The bolts use the flashing `flashcol` CRY index that `gamefx`
rebuilds every frame from the sine table, so they pulse through the palette
rather than holding one colour, with the original depth shading.
The sweep ends when no
eligible targets remain; it does not cancel future spawns. The second press
is a last-chance zap that hits one enemy and ignores enemy shots. That second
use has no lightning beam: the original clears its bolt flag before rendering.
Targets follow the original newest-first active-object list: the first eligible
object is zapped. On level one, the newest Flipper is usually the farthest down
the web, but the zapper does not compare distances.
Both uses recharge on web entry, including a retry after losing a life. Holding `C`
does not spend the second use automatically. An empty zap still uses a charge.
Initial launches on every level, including Key and direct `--level` starts, and
life-loss retries recharge silently. They follow `setweb`/`sweb`, which resets
`szap_avail` (`yak.s:8657-8659`) without announcing it. Only the `sweb0` path
after advancing or returning from a warp shows the recharge message and plays
its sound (`yak.s:8969-8974`). Announcement timing follows the load reason,
not the level number.

Zaps bypass Beastie armor and normal shot windows, and open Tankers normally.
Tankers' released enemies remain eligible for subsequent bolts. Mirrors,
climbing Alien Droids, approaching enemies, existing spikes, and reflected
shots are immune, matching the original collision routines. Zap kills award
points without dropping power-ups or advancing the pickup counter. The sixth
power-up starts the same full sweep without spending a manual use; if both
manual uses were spent, it restores the last-chance zap.

Shots launch from the current lane and travel straight down the tunnel.
They stay in their launch lane as the claw moves. The normal weapon fires
every four simulation ticks, with up to eight shots in flight, and removes
each shot at the rear opening. A full magazine skips that firing opportunity
and waits for the next one. Releasing Fire pauses the cooldown.

A shot is not a flat green dot. Sampling a reference screenshot shows three
layers: a near-white core in the shape of the original's `shot` vector, a
bright cyan-green body, and a darker green rim. The original gets this by
drawing a small shape at full brightness and letting bloom spread the colour,
so this port builds the layers into the mesh instead. The shades,
`(236, 250, 254)`, `(14, 250, 155)`, and `(0, 154, 53)`, are sampled from that
screenshot.

The shot's own colour register could not explain this. `frab` starts a shot at
hue `$88` and `player_shot` adds `$30` to the high byte of the colour word every
frame, but decoding that cycle through the Jaguar's CRY tables yields blues,
magentas, and oranges, never the green the original displays. The green is
therefore matched to the screenshot, not derived from the registers.

## First-level enemies

### Enemy appearance references

Enemy shapes must follow their active draw routines, not just similarly named
tables in `obj2d.s`. In particular, `draw_sfliptank` uses `s_fliptank2` and
`ft2verts` (`yak.s:2692-2704`, `obj2d.s:200-277`). The old `s_fliptank` pink
diamond is not the Tempest 2000 Flipper Tanker. The active model has thirteen
vertices and twelve faces, a 32-by-16 outline, red/blue crossed arms, and four
facets whose CRY hue follows `pulser(frames * 4)`. Its entire body rotates by
four Jaguar angle units per tick, making a full turn and color cycle in 64
ticks. The renderer caches those 64 color meshes and shares the phase between
Flipper Tankers; it does not rebuild meshes each frame.

The other active shape families were checked against their draw paths:
`draw_sfuseball` and `draw_sfusetank` use the two `fbpiece` leg shapes;
`draw_spulsar` and `draw_spulstank` use the six `spuls` poses, with an opposing
copy for Tankers; `draw_beast` layers `hornm1` through `hornm3`; Mirrors use
`mirr`, Alien Droids use `adroid`, and Spikers use the vector `spiker` spiral.
Fuseball arms consume `fbcols` backwards, from index four down to zero
(`yak.s:2596-2613`, `yak.s:2624-2647`), so their first arm is red, followed by
green, magenta, yellow, and cyan. The measured Flipper proportions and palette
adjustments described below remain intentional.

### Flippers

Level one uses the original `_nw1` wave of normal Flippers. Their X-shaped
bodies use `s_flipper`'s seven vertices and four triangular faces. The GPU
routine confirms the vertex table is indexed from zero, so the centre, the two
inner points, and the four tips map as listed.

Shading is matched to the original renderer rather than to a literal reading of
the data. `s_flipper` gives each face a CRY hue of `F0` or `F4` plus a
per-vertex intensity, which suggests a Gouraud gradient from dark arm tips to a
bright centre. Sampling a real screenshot shows otherwise: a Flipper renders
as four flat pure-red facets, with no green channel and no gradient along an
arm. So each face takes one flat shade from its brightest source intensity,
scaled so the brightest facet reaches 240 of red. This is a deliberate
departure from the raw data, chosen because it reproduces what the game
actually displays.

The proportions are adjusted for the same reason. A Flipper's four tips sit on
the corners of its bounding box, so that box decides the whole silhouette.
`fverts` uses 16 by 18, which is nearly square, so
the X closes towards a plus sign and each arm points at a corner. Measuring
both references shows a much shallower cross, with the arms meeting at about
58 degrees where ours meet at 97. A box twice as wide as it is tall gives 53
degrees, so this port
stretches the tips to `(0, 0)` and `(32, 16)`. That is the third deliberate
departure from `fverts`.

The inner vertices keep the source's proportion rather than its coordinates.
In `fverts` they sit halfway between the centre and a tip, which makes each arm
about a quarter as wide as it is long. The reference measures the same ratio,
so here they move to `(8, 8)` and `(24, 8)`, again halfway out.
Flippers first approach as small red points from beyond the rear opening.
Approach hues come from `draw_pel`'s `pixcols` table, independently of the
level's body palette: Flippers and Beasties use `$f0`, Tankers `$0f`, Fuseballs
`$80`, Pulsars `$ff`, Mirrors `$88`, and Alien Droids `$55`.
The point is a camera-facing square, kept at two original framebuffer pixels
across. This matches `draw_pel` and `snglpix`, which draw a 2-by-2 block with
depth-cued intensity rather than a shrinking, full-brightness sphere. It starts
dim and brightens as it approaches the web.
Their approach takes 150 ticks, then they climb their lane at the original
`0xA000` depth speed. At the rim they pivot about a lane boundary toward the
player, taking the shortest route around the closed web. A pivot advances
four of the Jaguar's 256 angle units per tick. Their between-flip pause counter
starts at eight and advances to the next flip on underflow. They
can be shot in the destination lane during the latter part of a flip. During
the first part they can grab a player in the departure lane. Once shootable,
they also threaten the destination lane. Shot hits resolve before grabs, so a
shot can save the player, but firing gaps leave the player exposed. Shot
collisions use the original strict six-unit depth range
at the current shot position, so the gaps between shots remain intact.
The weapon tracks active firing across the normal delay between shots, blocking
Flipper grabs until Fire is released or a firing attempt hits a full magazine.
After a magazine gap, the next emitted shot restores that protection. Enemy
shots still need to be intercepted or avoided.

The first spawn is after 61 ticks, with further spawns every 121 ticks when
there is room. The source's duration counter yields 25 enemies. Clearing all
of them completes the level. Each Flipper is worth 150 points.

Climbing Flippers also fire at the player. Level one allows one enemy shot
in flight. It uses `s_shot`'s three rotating chevrons, shaded with the original
purple and yellow-green CRY colors. Enemy shots travel toward the rim at
`0x18000` depth units per tick, and your shots can intercept them.
Climbing Flippers and Spikers share one 112-tick fire timer, as in `alienfire`.
Each eligible shooter decrements it once per tick in the original newest-first
active-object order, so the newest shooter fires first, and older shooters keep
counting down after a shot.
Their chevrons render at 0.15 world units per source pixel, twice the previous
mesh scale, to make the three darts easier to see against the filled web.
Each dart is moved inward by 1.5 source pixels before the three rotations,
reducing the projectile's outer radius from 1.2 to 0.975 world units while
preserving the individual chevron dimensions.
This is a presentation adjustment based on comparison with the original;
movement, four-unit rotation steps, lane/depth collisions, and firing limits
do not depend on that mesh scale.

Destroying a Flipper replaces it with the original's ring explosion. The
source turns the enemy into a `dpring` object, which draws two rings of single
pixels for 51 ticks: an inner ring of 16 pixels and an outer one of 14, at twice
the radius and four units further up the tube. Both rings start green and sweep
along `dat/sines.dat` through `pulser`, so the inner ring only reaches lime
while the outer one carries on into orange. Every frame of that sequence is
prebuilt here. Each explosion adjusts its dot geometry for camera distance and
projection, targeting one pixel at the original 384-pixel framebuffer width.
This keeps distant dots visible instead of shrinking them with the ring.

Opening a Tanker instead uses `tsphkillme` → `setsphk` → `drawsphere`'s particle
sphere. Sixteen rotating meridians contain 32 dots each, with real XYZ depth,
per-dot perspective sizing, an expanding radius, and fading CRY colors. Its
scale advances by two units per 60 Hz tick and expires at 64, as in `run_pixex`.

A Flipper grab shows "CAUGHT YOU" and locks movement, jumping, and firing.
Following `take_me_to_your_leader`, the Flipper and claw keep their grab poses
and travel down the web together at two depth units per tick, reaching the
rear after 80 ticks. The camera stays at the rim during this descent.
`snatch_it_away` then pulls the camera back at eight depth units per tick until
the original viewpoint threshold of -400, about 50 more ticks with this camera.
Only then is the life deducted and the wave reset. Restart in the pause menu cancels the sequence and
restarts immediately.

An enemy-shot hit shows "SHOT YOU!" and replaces the glider with `zap`'s
ten-point starburst outline. Its stroke uses shared mitered corners, keeping
the peaks sharp without overlapping rectangular segment ends.
`draw_z` draws four copies with hues separated by
17 palette entries. Following `zap_player`, the burst spins by 12 angle units
per tick, its copies spread apart by half a depth unit per tick, and its centre
moves toward the viewer by a quarter unit per tick. The camera moves toward
the burst's XY position and pulls back, including `vp_xform`'s depth correction.
The blue web faces fade out during the explosion.

After 121 ticks the starburst disappears, and the camera retreats at eight
depth units per tick until the same -400 viewpoint threshold as a grab. Only
then is the life deducted, followed by re-entry or game over. Movement, jumping,
and firing are locked during the sequence; Restart in the pause menu cancels it and restarts.

You start with four lives total. Losing a life resets the wave and gives two seconds
of invulnerability.
Choose **Restart** in the pause menu to restart on level one. After a level-clear exit, the next web loads
automatically.

The HUD warp-token pyramids have fixed red tips, green base corners, and white
central highlights, matching the [original-game screenshot on Wikipedia](https://en.wikipedia.org/wiki/File:JAG_Tempest_2000.png).
They do not pulse. This screenshot-based palette replaces `makepyr`'s animated
hue phases from the available assembly.

## Power-ups

Level one's `pup_stuff` entry is `$0404`, but `stdinit` clears the active byte.
`changex` decrements it and drops a power-up on underflow. The first kill drops
one, then kills 6, 11, 16, and 21. A drop replaces that enemy's normal explosion;
Intercepted enemy shots also advance the drop counter, following `run_ashot` →
`kill_ashot` → `killme` → `changex`. This supplies the extra drops needed to reach
the sixth pickup, EXCELLENT and a warp token, on web one. Superzap kills do not
advance the counter.

`draw_pup1` first shows the `opupring` formation animation: a contracting
20-dot ring and four expanding 24-dot rings. The pickup stays at the kill's
lane and depth for 76 ticks while `run_pup` reduces its formation radius from
300 by four each tick. It then travels toward the rim at `0x18000` depth units
per tick. Its body uses `pwrlaser`'s two lightning-bolt triangles, hue `$8a`,
and vertex intensities 128, 192, and 255. Four 12-dot rings trail it at eight-unit
depth intervals, rotating and changing colour through `pulser`. `src/cry.rs`
decodes these colours with all three Jaguar TOM channel tables.
The ring centers and cross sections are in web space, so the tube follows the
lane's depth axis under the same perspective as the web. Only the particle
dots and lightning glyph face the camera; dot size uses each ring's actual
view-space distance.

Collection requires the player to be grounded in the same lane, with the
pickup between zero and 32 depth units ahead. The bolt disappears, and the
four rings expand at five radius units per tick for 49 ticks. A missed pickup
passes the player and expires 80 units beyond the rim. Missing one leaves the
reward sequence unchanged.

| Collection | Reward |
| --- | --- |
| 1 | Particle laser: rotating rings of four color-cycling dots, shot speed 4.5 instead of 3.5 units per tick, a three-tick firing interval instead of four, and shots that pass through enemies |
| 2 | Surprise: normally 2,000 points; a 9-in-256 chance of 3,000 points and a level skip |
| 3 | Jump enabled: X lifts the claw off the rim, avoiding enemies and shots until landing |
| 4 | Another surprise bonus |
| 5 | AI droid: a rotating, colour-cycling wireframe cube that seeks the closest enemy, crosses one lane in 16 ticks, fires every six ticks using the player's current shot visuals, and adds eight bullet slots |
| 6 | Superzap plus a warp token |
| 7 onward | Surprise bonuses |

The surprise bonus uses the shared gameplay random stream, following `suprise`
and `rannum` (`yak.s:10628-10654`, `yak.s:17739-17745`). Rolls 0 through 8 award
3,000 points, OUTTA HERE and EXCELLENT, and start the level exit even with enemies
and unspawned enemies remaining. Other rolls award 2,000 points.

The particle laser uses `draw_pring` and the GPU's `pring`, not the four-wing
`pshot` polygon. Its dots stay two original framebuffer pixels wide, rotate
with camera-relative depth, and use `pulser` for their color sweep. Their rotation
uses the GPU's screen-down Y direction and byte sine table. Projected radii and
dot offsets truncate to original framebuffer pixels, as in `pring`, rather than
tracing smooth, oppositely wound spirals between pixels.

Jump presses are buffered between simulation ticks and work while firing.
The jump uses `sclawr`'s initial velocity of two units per tick and acceleration
`0xc11`; the camera follows its height. Losing a life removes the laser, jump,
and droid and restarts the collection sequence. Restarting from the pause menu clears all
power-ups. Remaining pickups do not delay level exit; they continue forming and
moving while the claw descends through the web. Collection uses their distance
ahead of the moving claw, rather than the rim, until the claw reaches the rear.

Collecting a pickup during exit displays and says YES! YES! YES! and reserves an
early AI droid for the next web's first collected pickup. It grants no points
and does not itself skip a level. The reserved droid survives web changes,
warp zones, and retries until collected; restarting clears it. Further pickups
during the same exit give surprise bonuses instead of reserving more droids.
The early droid leaves the regular reward index unchanged, so the next pickup
still grants the laser. Its later normal droid slot is skipped to superzap plus
a warp token. This follows `run_pup`'s `wave_tim == -2` and `dnt` branches at
`yak.s:10535-10576`. There is no direct 1UP reward in `pupvex`; points pickups can
earn lives by crossing the normal score thresholds.

Warp tokens accumulate across webs and survive deaths, up to three. The pickup
sequence and acquired laser, jump, and droid reset on every web entry, so later
webs can award additional tokens. R clears both upgrades and tokens. Level one
provides five drops from Flippers plus drops from intercepted shots; mixed waves can provide more
pickup-eligible kills through released Tanker contents.

## Animated messages

Typography candidates can be compared without changing the game using
`cargo run --example lettering_preview`. Add `-- --watch` to reload editable
SVG front outlines from `tools/lettering-preview.toml` without recompiling.
See [the lettering preview lab](../tools/lettering-preview.md) for comparison
sheets, individual PNGs, and runtime editing instructions.

Gameplay announcements use the approved candidate C lettering: rounded bowls
and terminals, slightly stronger medium strokes, and 17-unit character spacing.
Its SVG fronts are embedded from `assets/messages/rounded.toml`, including
matching letters and numerals for the full message catalog. The game and preview
lab share curve flattening and stroke-weight adjustment. Real bevels,
shallow extrusions, continuous blue-purple colour bands, and fixed normal-based
studio shading replace the old interpolated sprite shading. They render in a
native-resolution 3D overlay above the game and warp imagery. The ordinary
messages hold for the original 100-tick counter, then stretch vertically for
the 50-tick counter, including the source's inclusive expiry. Oscillating shear
is applied around each letter's own center. During expansion, the 3D glyph's
surfaces are divided into fixed-size cells. The opaque cells move apart while
retaining their size, opening real gaps that show the scene through the text.
The held letters remain joined and retain their reflective shading. The two-line recharge message
shares one timer and deformation. Spike avoidance and recharge use the longer
250-tick hold.

Pickups display the original stage wording, including ZAPPO!, OUTSTANDING!,
DUDE!, and the three warp-token messages. COLLECT POWERUPS! is limited to the
first four levels; AVOID THE SPIKES follows the source's levels 4–16 condition.
Death messages are emitted when the collision occurs, independently of the
small score/lives HUD. A new announcement replaces the active one, including
restarting an identical message's animation.

EXCELLENT, 2000, and 1UP use original fixed-size coloured cell rendering and
approach animation. 2000 omits cells with CRY intensity below 32 to trim its
darkest shading. Their cells separate
in both directions as they grow, keeping the playfield visible through the gaps.
1UP combines the original UP pixels with a half-sized 1 above them, excluding
CRY intensities at or below 80. Following `draw_oneup`, UP has one particle
plane while the numeral has six depth-separated copies. Their spacing grows
by a quarter unit each tick, with each plane projected independently. Cells
remain separate squares as the copies approach and spread apart.
The 250/500/750 score labels use their shorter cell expansion and fade.
They coexist with ordinary announcements. Score-based extra
lives trigger a 1UP effect for each award. Game over clears
ordinary messages and keeps the pulse and scale-and-rotate CRY feedback.
The 15 Hz feedback checkpoints are blended in decoded RGB at 60 Hz, preserving
their exact colors, trail buildup, pulse speed, and fade duration while smoothing
the intervening frames. Its lettering and fading trails use the previous bitmap
rendering; there is no separate sharp 3D foreground. The score stays visible until
restart.
Look-ahead feedback frames are cached between checkpoints rather than recomputed
for each blended frame. Clearing GAME OVER hides its cached canvas immediately,
including when entering or confirming initials while gameplay fixed updates are
stopped.

The tunnel instruction lasts 200 ticks at its original lower-screen anchor.
SPEED BOOST is shown in gate flight, matching its suppression in psychedelic
rings. WARP 5 LEVELS! follows the source victory timing and late-game display
conditions. The message catalog also provides previews for wording whose
mechanics or screens have not yet been implemented, such as practice completion
and final-game victory.

Preview a message without waiting for its gameplay trigger:

```sh
cargo run -- --message-preview particle-laser
cargo run -- --message-preview excellent --message-tick 30 --screenshot target/excellent.png
cargo run -- --message-preview particle-laser --message-tick 130 --message-size 3840x2160 --screenshot target/stretch-4k.png
cargo run -- --message-preview superzapper-recharge --warp-zone rings --screenshot target/recharge-over-rings.png
cargo run -- --message-preview one-up --record-motion target/one-up 4
```

`--message-preview` loops its selected animation. `--message-tick` freezes at
that exact animation age; screenshots default to tick 60. The game and star
simulation freeze with the message, and captures wait for the render assets
to settle. `--message-size WIDTHxHEIGHT` selects exact pixel dimensions with a
scale factor of one. `--help` lists the available message identifiers. Message
previews can accompany a direct warp scene or movement recording, but not an
exit/death recording. Restart clears messages, independent effects, and timers.

Assets are embedded; [assets/messages/README.md](../assets/messages/README.md) documents their provenance and
`tools/message_font.py` regenerates the contours from the original source tree.
The message renderer caches glyph volumes, fragment layouts, and linear colours.
Announcement glyphs and score digits are built at startup, because they appear
mid-game; the rainbow letters and gold pause lettering are built the first time
a menu or the pause screen shows them.
Unchanged frames, including paused or frozen previews, reuse the existing meshes.
The original approach effects keep their separate renderer, drawn through the overlay camera behind the HUD. A manual CPU benchmark is available with
`cargo test --release benchmark_recharge_rendering -- --ignored --nocapture`.

## Warp zones

All three warp zones are playable. Ready tokens are checked after
the normal exit and its flash, then consumed once when the bonus starts.
The upcoming web selects the zone in sixteen-level bands. Gate flight covers
upcoming webs 1–16 and 65–80; the green-track tunnel covers 17–32, 49–64,
and 81–96. Psychedelic rings cover 33–48 and 97–100. Natural bonus entry
is suppressed at the final two normal levels.

Fly automatically forward and steer with the arrow keys or W/A/S/D. Controls
use the original inertia, acceleration, and friction. The eight original
53-gate courses retain their spacing and special gates: up/down velocity
kicks, speed boosts, and a final pyramid. Every gate must be passed within
the original rectangular tolerance. Accurate passes award 750, 500, or 250
points. Missing a gate ends the bonus without losing a life; earned points
remain. Completion awards 20,000 points and advances four additional webs
when the pending destination is below level 91. The bonus returns through
normal web entry with fresh power-ups and restored cameras.

The original vertical control limits apply to powered steering, not coasting.
`iycon` spans ±320 raw units, while `m7run` divides them by eight for the camera,
giving a powered range of ±40 gate units (`yak.s:23906`, `yak.s:4863-4870`).
Higher and lower gates use the arrow gates' vertical velocity kicks. Release
the vertical input to coast beyond the limit; holding the outward direction
there clears velocity through `inertcon`'s `instop` branch. Use the opposite
direction to brake, then release or stop the inward motion near the next gate.
The reachability test drives the real keyboard input and bonus systems through
all 53 gates in every course at its first-cycle speed. It does not reposition
the player directly.

The scene uses the original eight-arm polygon-gate and icon layouts, a
crosshair, stars, and a scrolling Mode-7-style surface using the original
`beasty6.cry` texture. The surface lies at zero height, appearing overhead
when flying below it and beneath the player when flying above it. Course
order and difficulty progress independently of normal webs.
Gate flight draws the nearest three gates in full color and the next three with
progressively lower opacity, reducing visible pop-in on modern displays. Opacity
falls linearly with distance between the third gate and the hidden seventh gate;
all gates beyond the sixth are invisible. Icons share their gate's opacity.
Yellow up/down icons shade along the arrow's height, with matching colors at
shared triangle corners so their shafts have no diagonal seam.
The previews extend beyond the original 255-unit generation distance and
follow the generator's distance counters and current speed, including
rotation continuity when a preview becomes a live gate. Only rendering looks
ahead; gate checks, course spacing, boosts, and scoring retain their original
simulation timing. Gates on the opposite side of the surface remain hidden.
Rendering predicts only the six visible gates and their seventh-gate opacity
endpoint, rather than simulating previews for the remainder of the course.
Gate-score popups use readable numeral lettering split into fixed-size cells,
matching the square breakup effect of 2000 and EXCELLENT.
They stay centered on screen while steering, rather than projecting from a
near-camera world position. The extracted reward-sheet contours are unsuitable
for these three-digit labels.

The tunnel uses left/right or A/D to steer around its circular green track.
Its eight original 64-part courses feed a 64-segment buffer, with track
alignment checked against the segment under the player. Staying within the
original 32-angle-unit band grows a particle tail by half a unit per tick,
up to 31.5, and scores 1, 4, 8, or 30 points as the tail grows. Leaving the
track drains one unit per tick; exhausting it ends the bonus without a lost
life. Completion after 1,024 segments advances four additional webs below
level 91, without the gate-flight 20,000-point award. Colored particle arcs,
scrolling star planes, a trailing player marker, and the collapsing failure
tunnel reproduce the source drawing routines.
Tunnel failure has no explosion sound, matching the tail-exhaustion and collapse
paths in `yak.s:4281-4290` and `yak.s:4319-4330`.
An early failure removes the player tail and collapses the tunnel toward the
screen center for 32 ticks, then freezes that scene for the 151-tick `ofade`/
`ffade` bitmap melt before web entry (`yak.s:3964`, `yak.s:5069-5086`,
`yak.s:5297-5336`). The melt retains its scale-and-rotate trails and 60 Hz blended
presentation. This uses the existing 183-tick failure interval.
Successful completion holds the tunnel's 301-tick victory animation, hides the
separate WARP 5 LEVELS! text strip, and then applies the same 151-tick bitmap melt
before web entry (`yak.s:3959-3967`). The victory frame is frozen throughout the
melt.

Psychedelic rings use the same two-axis inertial controls as gate flight.
The original courses are mirrored horizontally, every ring uses the original
eight-block generation delay, and initial forward speed is `0xA000` rather
than gate flight's `0x8000`. Rings remain visible on both sides of the surface
height. Their particle outlines, special icons, and pulsating tilted sphere
are drawn into a 384×240 CRY feedback image that scales, rotates, and fades
each fixed tick. Passing rings brightens the feedback and cycles its rotation.
The source's ring-score lookup is preserved: combined integer axis errors
0–3, 4–7, 8–11, and 12–14 award 250, 500, 750, and 1 point respectively.
Missing a ring costs no life; completion awards 20,000 points and uses the
same extra-web advance rule as gate flight.

Gate flight and rings finish through `vicend`/`viccount`'s 151-tick,
zero-rotation scene fade, followed by the 401-tick `vicbon` screen
(`yak.s:5128–5134`, `yak.s:5273–5278`, `yak.s:4696–4799`). The crosshair stays
sharp during that first fade and disappears when the completion screen starts.
The original EXCELLENT / 20000 BONUS bitmap pulses and shears at `rex` depth 34
over `makepyr`'s glowing three-face pyramid. Both accumulate in a 384×240 CRY
feedback image. `versiondraw` uses scale 494/256, a triangular-wave rotation,
and a drifting source center; `bobo` sets the following frame's intensity loss
to 27, after the initial eight-unit loss (`yak.s:3023–3103`, `yak.s:4749–4788`).
Feedback checkpoints advance at 15 Hz and are blended at 60 Hz, as on GAME OVER.
The 60 Hz phase timers retain their existing timing.
WARP 5 LEVELS! stays sharp at the bottom of the screen when the destination
advances. Its top sits at logical row 202, matching the original display strip.
The final 151-tick `ofade`/`ffade` melt merges that lettering into the bitmap
before returning to web entry. Its initial rotation ranges from -3 through 4
brads and settles one step toward zero every four feedback frames. This
presentation-only choice uses the completion clock instead of consuming gameplay
randomness. The green-track tunnel retains its separate completion sequence.

Start a bonus directly for testing:

```sh
cargo run --release -- --warp-zone gate-flight
cargo run --release -- --warp-zone gate-flight --warp-course 8 --level 65
cargo run -- --warp-zone gate-flight --warp-course 1 --warp-gate 53
cargo run --release -- --warp-zone tunnel --warp-course 7 --level 17
cargo run --release -- --warp-zone rings --warp-course 3 --level 33
```

`--warp-zone gate` and `--warp-zone 1` select gate flight; `green-track` and
`2` select the tunnel; `psychedelic-rings` and `3` select rings. `--warp-course` chooses
the original course number from 1 to 8 and requires direct warp startup.
`--warp-gate 1..53` chooses the first gate to fly through in gate flight or rings.
It aligns the player with that gate and starts with zero steering velocity,
retaining earlier speed boosts but awarding no points for skipped gates. Omit it
for the ordinary course entry. Gate 53 is the final pyramid, so this command
lets you watch the completion animation without steering through a whole course.
The tunnel has no gates and rejects this option. Browser launches accept the
same `warp-gate` URL parameter, for example `?warp-zone=gate-flight&warp-gate=53`.
`--level` is the web reached after a failed direct-start run; success adds
four to that destination subject to the late-game restriction. Direct entry
does not require or consume tokens. Restart in the pause menu cancels the bonus and restarts level one.

Screenshot and recording options also work in a bonus scene:

```sh
cargo run -- --warp-zone gate-flight --screenshot target/gate-flight.png
cargo run -- --warp-zone gate-flight --record-motion target/gate-flight 30
```

The motion preview supplies scripted bonus steering. Bonus screenshots wait
for sixty fixed bonus ticks rather than for the hidden normal glider. Direct
warp motion recordings default to thirty seconds. Normal-web exit and shot-hit
previews are incompatible with direct warp startup.

## Background star animations

Normal play uses the original `siney` / `initpstarfield` background: 256 stars
whose X/Y sine frequencies are chosen from the current level and the original
random table. Their depth order and CRY colors follow the source, producing
different loops, woven curves, and spirals as the field moves toward the viewer.
Level 32 retains the original special frequency lookup. Both entries in the
gameplay `fields` table call `siney`, so every normal level uses its own pattern.
Web entry and retries rebuild the appropriate pattern while preserving the
moving depth phase; restart resets it. Bonus return regenerates the destination
web's pattern. The shared web/exit vanishing point is retained.

Entry uses the exit star-warp effect in reverse: the same exit motion supplies
the decreasing speed, while three additional depth samples contract with the
reversed exit trail spacing. The stars keep travelling toward the viewer until
the extra motion settles to zero and only the normal two-unit step remains.
Each new web or retry starts the effect again.

Other source variants can be selected for inspection:

```sh
cargo run --release -- --level 2
cargo run --release -- --level 32 --starfield level
cargo run --release -- --starfield rings
cargo run --release -- --starfield selection --level 12
```

The original random initializer, `initstarfield`, has no callers and is not
exposed here. The level pattern lookup values are embedded in Rust source.
The high-score ring field uses 256 stars in eight depth bands, a 1.75-unit motion
step, and seven samples with gradually increasing separation. The level-select
field uses fractional sine steps, six-unit motion, and five depth samples.
`--starfield` overrides the normal background; omitting it selects the original
level-driven animation. These options work with screenshots and recordings.

`gamefx` advances
the original star-field viewpoint by two units per tick, and `starfield1` in
`src/gpu/llama.gas` wraps relative depth over 512 units. Perspective makes the
stars spread outward faster as they approach, while their brightness rises
with proximity. Screen-sized square dots are drawn behind the web. Camera movement also
shifts their projected positions.

## Development

```sh
cargo fmt --check
cargo check
cargo test
cargo clippy -- -D warnings
```

Each Rust file starts with module documentation explaining its role and game
terminology. The source is organized by responsibility:

| Module | Contents |
| --- | --- |
| `src/main.rs`, `src/cli.rs` | Entry point and command-line options |
| `src/app.rs` | App construction and ordered input, fixed-tick simulation, and rendering schedules |
| `src/game.rs`, `src/hud.rs` | Game state, score, lives, level loading, retries, restarts, and status messages |
| `src/glider.rs`, `src/weapons.rs` | Player claw movement and poses, firing, bullets, and particle-laser graphics |
| `src/collisions.rs` | Shot hits, armor reflections, shell releases, spike trimming, and kill rewards |
| `src/scene.rs`, `src/camera.rs`, `src/mesh.rs` | Scene setup, web rendering, lane highlights, cameras, and shared mesh helpers |
| `src/web.rs`, `src/level.rs` | Lane topology and geometry, TOML definitions, validation, and the level catalog |
| `src/capture.rs` | Screenshots, recordings, scripted input, and transition previews |
| `src/enemies/` | Flippers, enemy shots, Tankers, Spikers and spikes, Beasties, Fuseballs, and Pulsars with rim sparks |
| `src/powerups/` | Reward state, pickups, particle rings, jumping, and the AI droid |
| `src/animations/` | Web entry, level exit and warp, Flipper capture, and glider death |
| `src/warp/` | Warp-zone routing, original gate courses, flight controls, scoring, bonus rendering, and return to normal webs |
| `src/explosion.rs`, `src/cry.rs`, `src/stars.rs` | Enemy-death rings, Jaguar color decoding, and background stars |
| `src/test_support.rs`, `src/**/tests/` | Headless fixtures and focused unit tests, compiled only for testing |

The game uses one loader for advancement, death retries, and restarts,
rebuilding the web and clearing level-owned actors each time. System ordering
is kept together in `app.rs` so movement, firing, collisions, and transitions
retain their fixed-tick timing.
Actors render in a separate camera pass over the web, matching the original
renderer's draw order and keeping complete enemy silhouettes visible.

To capture the level for visual comparisons, run:

```sh
cargo run -- --screenshot target/level-01.png
```

The app saves the window 60 visible-claw frames after the entry animation and
exits. The output directory must already exist.

To record a scripted movement comparison, run:

```sh
cargo run -- --record-motion target/motion
ffmpeg -framerate 30 -i target/motion/%04d.png -vf "pad=ceil(iw/2)*2:ceil(ih/2)*2" -c:v libx264 -pix_fmt yuv420p target/motion.mp4
```

The recording starts with level entry, then scripted movement and firing.
Its default duration is six seconds, including the 4.27-second web approach.
It saves screenshots roughly every two
simulation ticks, then exits. Use a fresh output directory for each recording.
The encoded video is a visual preview; screenshot timing can vary with GPU load.
An optional duration extends the capture, for example
`cargo run -- --record-motion target/enemies 15` to see enemies reach the rim.

To preview the exit without playing through the wave, run:

```sh
cargo run -- --record-exit target/exit
```

This records 14 seconds and clears the wave six seconds into the capture,
showing entry, the complete exit, the warp flash, and the start of re-entry.
An optional duration works as with `--record-motion`.

To preview the enemy-shot death sequence, run:

```sh
cargo run -- --record-shot-hit target/shot-hit
```

This records 15 seconds and injects an enemy shot six seconds into the capture,
showing entry, the starburst, camera retreat, and re-entry.

Gate-flight failures retain the missed-gate scene for the 151-tick `failcount`
interval. The renderer captures the scene once without UI, downsamples it to
384 by 240, and converts RGB to CRY through a five-bit-per-channel lookup built
at startup. This avoids a per-pixel hue search interrupting the transition. It uses
the same bitmap feedback operation as GAME OVER, rather than moving the camera.
`failfade`'s one-brad rotation, 500/256 sampling scale combined with the original
signed-byte sine table, and eight-unit intensity loss reprocess the previous
bitmap on each feedback frame (`yak.s:5027-5032`, `yak.s:5312-5336`). Nearest-neighbor
resampling accumulates the pixel smearing and distortion. Feedback retains its
15 Hz checkpoints, blended in decoded RGB at 60 Hz like GAME OVER, with the first
feedback step applied as soon as the capture arrives. The same smoothing applies
to the psychedelic rings' failure and final-gate fades. The failure timer retains
its 60 Hz timing. The crosshair stays visible and sharp above the feedback
throughout the failure. The
overlay is cleared on return to normal gameplay; delayed captures from an
interrupted failure cannot appear in a later session.

## Original geometry references

The original Tempest 2000 sources are available at
[mwenge/tempest2k](https://github.com/mwenge/tempest2k). The geometry references
used for this port are in `src/yak.s` and `src/obj2d.s`:

- `raw_webs` selects `web5` for the first level.
- `web5` defines the 16 ring vertices on a 2-to-14 coordinate grid.
- `extrude` repeats the same XY coordinates at a depth of 40 units.
- `vp_set` and `vp_xform` move the viewpoint toward the player's position.
- `clawcon` accelerates the claw and stops it before reversing direction.
- `domove` positions the claw at the selected lane's midpoint.
- `sclaw0` through `sclaw7` define the solid claw's movement poses.
- `setweb`, `rotate_web`, `srezclaw`, `dsclaw2`, and `draw_z` define level entry
  and the contracting wireframe claw trails.
- `szoom`, `zoom1`, `czoom1`, `zoom2`, and `czoom2` define the level-clear flight
  and expanding claw trails; `starfield1` draws the warp star samples.
- `dsclaw` selects one of those eight poses from the fractional lane position.
- `sclawr`, `frab`, and `player_shot` control repeat firing and shot travel.
- `_nw1`, `setnewgen`, and `newgen` define level-one enemies and spawn timing.
- `s_flipper` and `fverts` define the solid Flipper faces and vertices.
- `make_flipper`, `ipix`, and `r_obj` define the distant-point approach.
- `fz_stuff`, `rospeeds`, and `pauses` define the level-one movement parameters.
- `stdrail`, `flip_set`, `flipto`, and `stdstop` implement climbing and rim pivots.
- `collie` defines shot vulnerability and the six-unit depth collision threshold.
- `gotu`, `take_me_to_your_leader`, and `snatch_it_away` define the Flipper grab,
  joint descent, camera retreat, and delayed loss of a life.
- `run_ashot`, `ouch`, `zap_player`, `zap`, and `draw_z` define the shot-hit
  message, spinning starburst, depth copies, and retreat before losing a life.
- `scores` assigns 150 points to a normal Flipper.
- `alienfire`, `run_ashot`, `s_shot`, and `chevron` define enemy firing and shots.
- `killme`, `sphkillme`, and `kime` turn a killed Flipper into its explosion.
- `dpring`, `draw_prex`, and `run_prex` build, draw, and age the ring explosion.
- `pulser` reads `dat/sines.dat` to pick each ring's colour.
- `pring2` in `src/gpu/xcamel.gas` plots a ring a pixel at a time.
- `pup_stuff`, `changex`, and `run_pup` control drops and collection.
- `draw_pup1`, `opupring`, `pupring`, and `pwrlaser` define the pickup visuals.
- `pupvex`, `hilaser`, `suprise`, `jenab`, `addroid`, and `sprzap` define rewards.
- `sclawr`, `mkdroid`, and `rundroid` implement jumping and droid assistance.

`pring2` also computes `255 - radius >> 16` as a ring's intensity, which looks
like a fade as the ring grows. It cannot be one: by that point it has already
divided the radius by z, so the shifted byte is always zero and every ring draws
at full intensity. The colour sweep is the whole of the apparent fade.

The CRY hue-to-RGB lookup used to check those colors comes from
[Virtual Jaguar's TOM implementation](https://github.com/libretro/virtualjaguar-libretro/blob/master/src/tom/tom.c).
The tables are adapted under GPLv3; upstream credits are preserved in [NOTICE](../NOTICE).
It decodes `F4` to a red-orange, but the reference screenshot shows no green on
a Flipper at all, which is why this port does not use that value.
Virtual Jaguar's red row 8 has two typos (`15` and `7`); they are corrected to
`115` and `17`, matching MAME's `red_lookup` and the mirrored blue row 7.

The camera framing and colors are tuned to the supplied screenshot rather
than reproducing the Jaguar's renderer exactly.
