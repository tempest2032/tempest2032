#!/usr/bin/env python3
"""Launch a packaged game outside the source tree and require rendered captures."""

import argparse
from pathlib import Path
import struct
import subprocess
import tempfile


def capture(command, output, name, options, timeout):
    screenshot = output / f"{name}.png"
    screenshot.unlink(missing_ok=True)
    log = output / f"{name}.log"
    # No levels/ or assets/ alongside the working directory can hide packaging errors.
    with tempfile.TemporaryDirectory(prefix="tempest2032-smoke-") as working:
        with log.open("wb") as stream:
            subprocess.run(
                [*command, *options, "--screenshot", str(screenshot)],
                cwd=working, stdout=stream, stderr=subprocess.STDOUT,
                check=True, timeout=timeout,
            )
    data = screenshot.read_bytes()
    if (len(data) < 45 or data[:8] != b"\x89PNG\r\n\x1a\n"
            or data[12:16] != b"IHDR" or data[-8:] != b"IEND\xaeB`\x82"
            or 0 in struct.unpack(">II", data[16:24])):
        raise RuntimeError(f"Game did not produce a complete PNG: {screenshot}")
    print(f"Rendered {name}: {screenshot}", flush=True)


def run(command, output, timeout=120):
    output = Path(output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    capture(command, output, "welcome", ["--menu-preview", "welcome"], timeout)
    capture(command, output, "gameplay", ["--level", "1"], timeout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command
    if command[:1] == ["--"]:
        command = command[1:]
    if not command:
        parser.error("Provide the game command after --")
    run(command, args.output)


if __name__ == "__main__":
    main()
