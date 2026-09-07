"""Exercise the real installer with fake network responses and isolated paths."""
import hashlib
import io
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest

INSTALLER = Path(__file__).resolve().parents[2] / "install.sh"


class InstallerTests(unittest.TestCase):
    def install(self, *, corrupt=False, occupied=False):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fake = root / "fake"
            fake.mkdir()
            archive = root / "archive.tar.gz"
            with tarfile.open(archive, "w:gz") as output:
                for name, content in {
                    "bin/jcodex": b"#!/bin/sh\necho 'jcodex 0.1.0'\n",
                    "codex-package.json": b'{"version":"0.1.0"}',
                }.items():
                    info = tarfile.TarInfo(name)
                    info.size = len(content)
                    info.mode = 0o755
                    output.addfile(info, io.BytesIO(content))
            checksum = "0" * 64 if corrupt else hashlib.sha256(archive.read_bytes()).hexdigest()
            (root / "SHA256SUMS").write_text(checksum + "  jcodex-aarch64-apple-darwin.tar.gz\n")
            (fake / "uname").write_text('#!/bin/sh\ncase "$1" in -m) echo arm64;; *) echo Darwin;; esac\n')
            (fake / "curl").write_text('''#!/usr/bin/env python3
import os, shutil, sys
from pathlib import Path
args = sys.argv[1:]
if '-o' not in args:
    print('{"tag_name": "jcodex-v0.1.0"}')
else:
    url = next(a for a in args if a.startswith('https://'))
    source = 'SHA256SUMS' if url.endswith('SHA256SUMS') else 'archive.tar.gz'
    shutil.copyfile(Path(os.environ['FIXTURE']) / source, args[args.index('-o') + 1])
''')
            for path in fake.iterdir():
                path.chmod(0o755)
            bindir = root / "bin"
            bindir.mkdir()
            (bindir / "codex").write_text("upstream")
            if occupied:
                (bindir / "jcodex").write_text("existing")
            env = dict(os.environ, PATH=str(fake) + os.pathsep + os.environ["PATH"],
                       FIXTURE=str(root), JCODEX_INSTALL_ROOT=str(root / "packages"),
                       JCODEX_BIN_DIR=str(bindir))
            env.pop("JCODEX_VERSION", None)
            result = subprocess.run(["bash", str(INSTALLER)], env=env, text=True, capture_output=True)
            self.assertEqual((bindir / "codex").read_text(), "upstream")
            if corrupt or occupied:
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((bindir / "jcodex").is_symlink())
            else:
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertTrue((bindir / "jcodex").is_symlink())
                repeated = subprocess.run(["bash", str(INSTALLER)], env=env, capture_output=True)
                self.assertEqual(repeated.returncode, 0, repeated.stderr)

    def test_install_and_repeat_preserve_codex(self):
        self.install()

    def test_reject_bad_checksum(self):
        self.install(corrupt=True)

    def test_preserve_existing_non_symlink(self):
        self.install(occupied=True)


if __name__ == "__main__":
    unittest.main()
