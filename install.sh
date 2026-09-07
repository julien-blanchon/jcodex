#!/usr/bin/env bash
# Installs a complete jcodex package without modifying Codex or its configuration.
set -euo pipefail
main() {
  local repo="julien-blanchon/jcodex" target arch tag base archive tmp checksum actual destination link_dir
  case "$(uname -m)" in
    arm64|aarch64) arch=aarch64 ;;
    x86_64|amd64) arch=x86_64 ;;
    *) echo "Unsupported architecture" >&2; return 1 ;;
  esac
  case "$(uname -s)" in
    Darwin) target="$arch-apple-darwin" ;;
    Linux) target="$arch-unknown-linux-musl" ;;
    *) echo "Use the Windows ZIP from https://github.com/$repo/releases" >&2; return 1 ;;
  esac
  tag="${JCODEX_VERSION:-}"
  if [[ -z "$tag" ]]; then
    tag=$(curl -fsSL --proto '=https' "https://api.github.com/repos/$repo/releases/latest" |
      sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p')
  fi
  [[ "$tag" =~ ^jcodex-v[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "Invalid release tag: $tag" >&2; return 1; }
  base="https://github.com/$repo/releases/download/$tag"
  archive="jcodex-$target.tar.gz"
  tmp=$(mktemp -d)
  # Capture the function-local temporary path when registering cleanup.
  # shellcheck disable=SC2064
  trap "rm -rf -- $(printf '%q' "$tmp")" EXIT
  curl -fsSL --proto '=https' "$base/$archive" -o "$tmp/$archive"
  curl -fsSL --proto '=https' "$base/SHA256SUMS" -o "$tmp/SHA256SUMS"
  checksum=$(awk -v name="$archive" '$2 == name {print $1}' "$tmp/SHA256SUMS")
  [[ "$checksum" =~ ^[a-f0-9]{64}$ ]] || { echo "Missing checksum for $archive" >&2; return 1; }
  if command -v sha256sum >/dev/null; then
    actual=$(sha256sum "$tmp/$archive")
  else
    actual=$(shasum -a 256 "$tmp/$archive")
  fi
  [[ "${actual%% *}" == "$checksum" ]] || { echo "Checksum mismatch" >&2; return 1; }
  mkdir "$tmp/package"
  tar -xzf "$tmp/$archive" -C "$tmp/package"
  [[ -x "$tmp/package/bin/jcodex" && -f "$tmp/package/codex-package.json" ]] || { echo "Invalid package" >&2; return 1; }
  destination="${JCODEX_INSTALL_ROOT:-$HOME/.local/share/jcodex}/$tag-$target"
  link_dir="${JCODEX_BIN_DIR:-$HOME/.local/bin}"
  mkdir -p "$(dirname "$destination")" "$link_dir"
  if [[ -e "$link_dir/jcodex" && ! -L "$link_dir/jcodex" ]]; then
    echo "Refusing to replace a non-symlink: $link_dir/jcodex" >&2; return 1
  fi
  if [[ ! -e "$destination" ]]; then
    mv "$tmp/package" "$destination"
  fi
  [[ -x "$destination/bin/jcodex" && -f "$destination/codex-package.json" ]] || { echo "Incomplete existing package: $destination" >&2; return 1; }
  "$destination/bin/jcodex" --version
  ln -sfn "$destination/bin/jcodex" "$link_dir/jcodex"
  echo "Installed $link_dir/jcodex. Add $link_dir to PATH if needed."
  rm -rf "$tmp"
  trap - EXIT
}
main "$@"
