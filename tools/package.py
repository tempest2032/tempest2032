#!/usr/bin/env python3
"""Build native packages or prepare an offline Flatpak/Flathub source manifest."""

import argparse
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import tarfile
import tempfile
import tomllib

import smoke

ROOT = Path(__file__).resolve().parents[1]
APP_ID = "io.github.tempest2032.Tempest2032"
VERSION = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]


def copy_binary(binary, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(binary, destination)
    destination.chmod(0o755)


def native(platform, target, binary, output, smoke_test=False):
    binary = binary.resolve(strict=True)
    with tempfile.TemporaryDirectory(dir=output) as temporary:
        stage = Path(temporary)
        if platform == "windows":
            compiler = shutil.which("ISCC")
            if not compiler:
                compiler = Path(os.environ["ProgramFiles(x86)"]) / "Inno Setup 6" / "ISCC.exe"
            subprocess.run([
                str(compiler), f"/DAppVersion={VERSION}", f"/DBinaryPath={binary}",
                f"/DIconPath={ROOT / 'packaging/windows/icon.ico'}",
                f"/DOutputDir={output}", str(ROOT / "packaging/windows/installer.iss"),
            ], check=True)
            installer = output / f"tempest2032-{VERSION}-windows-x86_64-setup.exe"
            installed = stage / "installed"
            subprocess.run([
                str(installer), "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/SP-",
                f"/DIR={installed}", "/GROUP=Tempest 2032 package check", "/TASKS=",
            ], check=True, timeout=120)
            try:
                if (installed / "tempest2032.exe").read_bytes() != binary.read_bytes():
                    raise RuntimeError("Installer did not preserve the standalone executable")
                if smoke_test:
                    smoke.run([str(installed / "tempest2032.exe")], output / "smoke")
            finally:
                subprocess.run([
                    str(installed / "unins000.exe"), "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART",
                ], check=True, timeout=120)
        elif platform == "macos":
            contents = stage / "Tempest 2032.app" / "Contents"
            copy_binary(binary, contents / "MacOS/tempest2032")
            subprocess.run([
                "rustc", "--edition=2024", "-C", "opt-level=s", "-C", "strip=symbols",
                str(ROOT / "packaging/fullscreen.rs"),
                "-o", str(contents / "MacOS/tempest2032-fullscreen"),
            ], check=True)
            resources = contents / "Resources"
            resources.mkdir()
            shutil.copyfile(ROOT / "packaging/macos/icon.icns", resources / "icon.icns")
            for name in ["LICENSE", "NOTICE"]:
                shutil.copyfile(ROOT / name, resources / name)
            with (contents / "Info.plist").open("wb") as file:
                plistlib.dump({
                    "CFBundleIdentifier": APP_ID,
                    "CFBundleName": "Tempest 2032",
                    "CFBundleDisplayName": "Tempest 2032",
                    "CFBundleExecutable": "tempest2032-fullscreen",
                    "CFBundleIconFile": "icon.icns",
                    "CFBundlePackageType": "APPL",
                    "CFBundleShortVersionString": VERSION,
                    "CFBundleVersion": VERSION,
                    "LSMinimumSystemVersion": "11.0",
                    "NSHighResolutionCapable": True,
                }, file)
            subprocess.run(["codesign", "--force", "--sign", "-", str(contents / "MacOS/tempest2032")], check=True)
            subprocess.run(["codesign", "--force", "--sign", "-", str(contents.parent)], check=True)
            subprocess.run(["codesign", "--verify", "--strict", str(contents.parent)], check=True)
            if smoke_test:
                smoke.run([str(contents / "MacOS/tempest2032-fullscreen")], output / "smoke")
            (stage / "Applications").symlink_to("/Applications")
            subprocess.run([
                "hdiutil", "create", "-volname", "Tempest 2032", "-srcfolder", str(stage),
                "-format", "UDZO", str(output / f"tempest2032-{VERSION}-macos-arm64.dmg"),
            ], check=True)
            subprocess.run([
                "hdiutil", "verify", str(output / f"tempest2032-{VERSION}-macos-arm64.dmg"),
            ], check=True)
        else:
            name = f"tempest2032-{VERSION}-linux-{target.split('-')[0]}"
            folder = stage / name
            copy_binary(binary, folder / "tempest2032")
            shutil.copyfile(ROOT / "assets/steam/icon.png", folder / "icon.png")
            for filename in ["LICENSE", "NOTICE"]:
                shutil.copyfile(ROOT / filename, folder / filename)
            (folder / "README.txt").write_text(
                "Tempest 2032\n\nCopy this folder anywhere and run ./tempest2032.\n"
                "All game assets and levels are included in the executable.\n"
                "Requires glibc 2.39 or newer, a graphical desktop, a Vulkan-capable\n"
                "graphics driver, and the system ALSA, udev, X11/Wayland libraries.\n"
                "Arrow keys or WASD move; Z/Space fires; X jumps; C zaps; P pauses.\n"
                "Escape returns to Welcome during play, and quits on Welcome.\n"
                "Licensed under GPLv3; see LICENSE and NOTICE.\n"
                "Run ./tempest2032 --help for options.\n"
            )
            with tarfile.open(output / f"{name}.tar.gz", "w:gz") as archive:
                archive.add(folder, arcname=name)
            if smoke_test:
                smoke.run([str(folder / "tempest2032")], output / "smoke")


def flatpak(output, revision):
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError("Flatpak source revision must be a full 40-character commit ID")
    manifest = json.loads((ROOT / f"packaging/linux/{APP_ID}.json").read_text())
    sources = []
    for package in tomllib.loads((ROOT / "Cargo.lock").read_text())["package"]:
        if "source" not in package:
            continue
        if package["source"] != "registry+https://github.com/rust-lang/crates.io-index":
            raise ValueError(f"Unsupported Cargo source: {package['source']}")
        name, version, checksum = package["name"], package["version"], package["checksum"]
        destination = f"cargo/vendor/{name}-{version}"
        sources.extend([
            {
                "type": "archive", "archive-type": "tar-gzip",
                "url": f"https://static.crates.io/crates/{name}/{name}-{version}.crate",
                "sha256": checksum, "dest": destination,
            },
            {
                "type": "inline", "dest": destination,
                "dest-filename": ".cargo-checksum.json",
                "contents": json.dumps({"package": checksum, "files": {}}),
            },
        ])
    sources.append({
        "type": "inline", "dest": "cargo", "dest-filename": "config.toml",
        "contents": '[source.crates-io]\nreplace-with = "vendored-sources"\n'
                    '[source.vendored-sources]\ndirectory = "cargo/vendor"\n',
    })
    (output / "cargo-sources.json").write_text(json.dumps(sources, indent=2) + "\n")
    manifest["modules"][0]["sources"] = [
        {"type": "git", "url": "https://github.com/tempest2032/tempest2032.git", "commit": revision},
        "cargo-sources.json",
    ]
    (output / f"{APP_ID}.json").write_text(json.dumps(manifest, indent=2) + "\n")
    with tarfile.open(output / "flathub-submission.tar.gz", "w:gz") as archive:
        for name in [f"{APP_ID}.json", "cargo-sources.json"]:
            archive.add(output / name, arcname=name)
    # Build the checked-out source too, including pull requests not yet merged upstream.
    source = output / "source"
    source.mkdir()
    for name in ["src", "assets", "levels", "packaging"]:
        shutil.copytree(ROOT / name, source / name)
    for name in ["Cargo.toml", "Cargo.lock", "build.rs", "LICENSE", "NOTICE"]:
        shutil.copyfile(ROOT / name, source / name)
    manifest["modules"][0]["sources"][0] = {"type": "dir", "path": "source"}
    (output / "local.json").write_text(json.dumps(manifest, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("platform", choices=["windows", "macos", "linux", "flatpak"])
    parser.add_argument("--target")
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--revision")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--smoke-test", action="store_true", help="Launch native packages and verify screenshots")
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    if args.platform == "flatpak":
        if not args.revision:
            parser.error("Flatpak packaging requires --revision")
        flatpak(output, args.revision)
    else:
        if not args.target or not args.binary:
            parser.error("Native packaging requires --target and --binary")
        native(args.platform, args.target, args.binary, output, smoke_test=args.smoke_test)


if __name__ == "__main__":
    main()
