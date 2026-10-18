import base64
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import smoke


PNG = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aX1cAAAAASUVORK5CYII="
)


class Smoke(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.output = Path(self.temp.name)

    def test_both_scenes_run_outside_source_tree_and_keep_logs(self):
        script = (
            "import pathlib,sys; "
            "print(pathlib.Path.cwd(), flush=True); "
            "print(sys.argv, flush=True); "
            f"pathlib.Path(sys.argv[-1]).write_bytes({PNG!r})"
        )
        smoke.run([sys.executable, "-c", script], self.output)
        for name, option in [("welcome", "--menu-preview"), ("gameplay", "--level")]:
            self.assertEqual((self.output / f"{name}.png").read_bytes(), PNG)
            log = (self.output / f"{name}.log").read_text()
            self.assertIn(option, log)
            self.assertNotEqual(Path(log.splitlines()[0]), Path.cwd())

    def test_successful_exit_cannot_reuse_a_stale_screenshot(self):
        screenshot = self.output / "gameplay.png"
        screenshot.write_bytes(PNG)
        with self.assertRaises(FileNotFoundError):
            smoke.capture([sys.executable, "-c", "pass"], self.output, "gameplay", [], 5)
        self.assertFalse(screenshot.exists())

    def test_crash_and_hang_fail_the_check_and_keep_diagnostics(self):
        for script, error in [
            ("import sys; print('startup failed', flush=True); sys.exit(1)", subprocess.CalledProcessError),
            ("import time; print('stuck', flush=True); time.sleep(60)", subprocess.TimeoutExpired),
        ]:
            with self.subTest(script=script), self.assertRaises(error):
                smoke.capture([sys.executable, "-c", script], self.output, "gameplay", [], 1)
            self.assertTrue((self.output / "gameplay.log").read_text())

    def test_truncated_image_fails_even_after_successful_exit(self):
        script = "import pathlib,sys; pathlib.Path(sys.argv[-1]).write_bytes(b'\\x89PNG\\r\\n\\x1a\\n')"
        with self.assertRaisesRegex(RuntimeError, "complete PNG"):
            smoke.capture([sys.executable, "-c", script], self.output, "gameplay", [], 5)


if __name__ == "__main__":
    unittest.main()
