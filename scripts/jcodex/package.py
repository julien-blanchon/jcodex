#!/usr/bin/env python3
"""Apply the fork's entrypoint rename to a validated upstream runtime package."""
import argparse
import json
import shutil
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from codex_package.archive import write_archive


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path)
    parser.add_argument("archive", type=Path)
    args = parser.parse_args()
    manifest_path = args.package / "codex-package.json"
    manifest = json.loads(manifest_path.read_text())
    source = args.package / manifest["entrypoint"]
    target = source.with_name("jcodex" + source.suffix)
    source.rename(target)
    manifest["entrypoint"] = target.relative_to(args.package).as_posix()
    manifest["distribution"] = "jcodex"
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
    root = Path(__file__).resolve().parents[2]
    for name in ("LICENSE", "NOTICE"):
        shutil.copyfile(root / name, args.package / name)
    write_archive(args.package, args.archive.resolve(), force=False)


if __name__ == "__main__":
    main()
