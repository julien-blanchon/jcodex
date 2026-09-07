#!/usr/bin/env python3
"""Generate the jcodex formula from published release checksums (no dependencies)."""

import json
import re
import sys
from urllib.request import urlopen

REPO = "julien-blanchon/jcodex"


def generate(tag, checksums):
    if not re.fullmatch(r"jcodex-v[0-9]+\.[0-9]+\.[0-9]+", tag):
        raise ValueError("Invalid release tag")
    hashes = dict(line.split()[::-1] for line in checksums.splitlines())
    lines = [
        "class Jcodex < Formula",
        '  desc "Codex fork with event-driven command monitors"',
        f'  homepage "https://github.com/{REPO}"',
        f'  version "{tag.removeprefix("jcodex-v")}"',
        '  license "Apache-2.0"',
        "",
    ]
    for os_name, triple in [("macos", "apple-darwin"), ("linux", "unknown-linux-musl")]:
        lines.append(f"  on_{os_name} do")
        for cpu, arch in [("arm", "aarch64"), ("intel", "x86_64")]:
            asset = f"jcodex-{arch}-{triple}.tar.gz"
            digest = hashes[asset]
            if not re.fullmatch(r"[0-9a-f]{64}", digest):
                raise ValueError(f"Invalid checksum for {asset}")
            lines.extend(
                [
                    f"    on_{cpu} do",
                    f'      url "https://github.com/{REPO}/releases/download/{tag}/{asset}"',
                    f'      sha256 "{digest}"',
                    "    end",
                ]
            )
        lines.extend(["  end", ""])
    lines.extend(
        [
            "  def install",
            '    libexec.install "bin", "codex-resources", "codex-path", "codex-package.json"',
            '    bin.install_symlink libexec/"bin/jcodex"',
            "  end",
            "",
            "  test do",
            '    assert_match version.to_s, shell_output("#{bin}/jcodex --version")',
            '    assert_match "monitor", shell_output("#{bin}/jcodex features list")',
            "  end",
            "end",
        ]
    )
    return "\n".join(lines) + "\n"


def main():
    tag = (
        sys.argv[1]
        if len(sys.argv) > 1
        else json.load(
            urlopen(f"https://api.github.com/repos/{REPO}/releases/latest", timeout=30)
        )["tag_name"]
    )
    with urlopen(
        f"https://github.com/{REPO}/releases/download/{tag}/SHA256SUMS", timeout=30
    ) as response:
        checksums = response.read().decode()
    print(generate(tag, checksums), end="")


if __name__ == "__main__":
    main()
