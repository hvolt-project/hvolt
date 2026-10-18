#!/bin/sh
# hvolt installer: downloads the prebuilt binary for this platform from the
# GitHub release, verifies it against SHA256SUMS.txt, and installs it.
#
#   curl -fsSL https://raw.githubusercontent.com/hvolt-project/hvolt/main/install.sh | sh
#
# Environment:
#   HVOLT_VERSION  version to install without the leading "v" (default: latest)
#   HVOLT_PREFIX   install directory (default: $HOME/.local/bin)
set -eu

REPO="hvolt-project/hvolt"
VERSION="${HVOLT_VERSION:-latest}"
PREFIX="${HVOLT_PREFIX:-$HOME/.local/bin}"

die() { echo "hvolt: $*" >&2; exit 1; }

os=$(uname -s 2>/dev/null | tr '[:upper:]' '[:lower:]')
arch=$(uname -m 2>/dev/null)
case "$os" in
  darwin)
    case "$arch" in
      arm64|aarch64) key="darwin-arm64" ;;
      *) die "no prebuilt binary for macOS $arch (supported: arm64)" ;;
    esac ;;
  linux)
    case "$arch" in
      x86_64|amd64) key="linux-x64" ;;
      *) die "no prebuilt binary for Linux $arch (supported: x86_64)" ;;
    esac ;;
  *)
    die "unsupported OS $os; on Windows use install.ps1" ;;
esac

# Release assets are versioned, so `latest` has to be resolved to its tag first.
if [ "$VERSION" = "latest" ]; then
  # The HTML redirect from /releases/latest lands on /releases for anonymous
  # requests, so resolve the tag through the API and fall back to the Location
  # header (which does carry the tag).
  tag=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
    | grep -m1 '"tag_name"' | cut -d'"' -f4)
  if [ -z "$tag" ]; then
    tag=$(curl -fsSI "https://github.com/$REPO/releases/latest" \
      | tr -d '\r' | sed -n 's|^[Ll]ocation: .*/tag/\(.*\)$|\1|p')
  fi
  [ -n "$tag" ] || die "could not resolve the latest release"
else
  tag="v$VERSION"
fi
version="${tag#v}"
base="https://github.com/$REPO/releases/download/$tag"

archive="hvolt-$version-$key.tar.gz"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM

# Asset downloads: try the release URL first, then fall back to the API asset
# endpoint, which is reachable where github.com itself is blocked (measured on a
# network where github.com returns no connection but api.github.com returns 200).
fetch_asset() {
  name=$1
  out=$2
  if curl -fsSL "$base/$name" -o "$out" 2>/dev/null; then return 0; fi
  asset_id=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/tags/$tag" \
    | tr -d '\n' \
    | awk -v want="$name" '
        { while (match($0, /"id": *[0-9]+/)) {
            id = substr($0, RSTART, RLENGTH); sub(/.*: */, "", id);
            $0 = substr($0, RSTART + RLENGTH);
          }
          if (match($0, /"name": *"[^"]*"/)) {
            n = substr($0, RSTART, RLENGTH); sub(/.*"name": *"/, "", n); sub(/"$/, "", n);
            if (n == want && id != "") { print id; exit }
          }
        }')
  [ -n "$asset_id" ] || return 1
  curl -fsSL -H "Accept: application/octet-stream" \
    "https://api.github.com/repos/$REPO/releases/assets/$asset_id" -o "$out" 2>/dev/null
}

echo "hvolt: fetching $archive"
fetch_asset "$archive" "$tmp/$archive" || die "could not download $archive"
fetch_asset "SHA256SUMS.txt" "$tmp/SHA256SUMS.txt" || die "could not download SHA256SUMS.txt"

if command -v sha256sum >/dev/null 2>&1; then
  hash_cmd="sha256sum"
elif command -v shasum >/dev/null 2>&1; then
  hash_cmd="shasum -a 256"
else
  die "need sha256sum or shasum to verify the download"
fi

expected=$(awk -v f="$archive" '{ n = $2; sub(/.*\//, "", n); if (n == f) { print $1; exit } }' \
  "$tmp/SHA256SUMS.txt")
[ -n "$expected" ] || die "$archive is not listed in SHA256SUMS.txt"
actual=$($hash_cmd "$tmp/$archive" | cut -d' ' -f1)
[ "$expected" = "$actual" ] || die "checksum mismatch for $archive (expected $expected, got $actual)"

tar -xzf "$tmp/$archive" -C "$tmp"
mkdir -p "$PREFIX"
install -m 755 "$tmp/hvolt" "$PREFIX/hvolt" 2>/dev/null || {
  cp "$tmp/hvolt" "$PREFIX/hvolt"; chmod 755 "$PREFIX/hvolt"; }

echo "hvolt: installed $("$PREFIX/hvolt" --version) to $PREFIX/hvolt"
case ":$PATH:" in
  *":$PREFIX:"*) ;;
  *) echo "hvolt: add $PREFIX to PATH to run it by name" ;;
esac
