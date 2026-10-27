#!/bin/sh
# pluto installer: downloads the latest release binary into ~/.local/bin
#
#   curl -fsSL https://raw.githubusercontent.com/darlingson/pluto/main/install.sh | bash
#
set -eu

repo="darlingson/pluto"

case "$(uname -s)" in
    Linux)
        ;;
    *)
        echo "pluto installer: no prebuilt binary for $(uname -s)." >&2
        echo "install from source instead:" >&2
        echo "  cargo install --git https://github.com/$repo" >&2
        exit 1
        ;;
esac

case "$(uname -m)" in
    x86_64)
        arch="x86_64"
        ;;
    *)
        echo "pluto installer: no prebuilt binary for $(uname -m)." >&2
        echo "install from source instead:" >&2
        echo "  cargo install --git https://github.com/$repo" >&2
        exit 1
        ;;
esac

if ! command -v curl >/dev/null 2>&1; then
    echo "pluto installer: curl is required" >&2
    exit 1
fi

release=$(curl -fsSL "https://api.github.com/repos/$repo/releases/latest") || {
    echo "pluto installer: could not reach GitHub releases" >&2
    exit 1
}
tag=$(printf '%s' "$release" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)
if [ -z "$tag" ]; then
    echo "pluto installer: could not determine latest release" >&2
    exit 1
fi
version="${tag#v}"
asset="pluto-${version}-${arch}-linux.tar.gz"
url="https://github.com/$repo/releases/download/${tag}/${asset}"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
curl -fsSL -o "$tmp/$asset" "$url"
tar xzf "$tmp/$asset" -C "$tmp"

dest="${PLUTO_INSTALL_DIR:-$HOME/.local/bin}"
mkdir -p "$dest"
mv "$tmp/pluto" "$dest/pluto"
chmod +x "$dest/pluto"

echo "installed $dest/pluto ($tag)"
case ":$PATH:" in
    *":$dest:"*) ;;
    *)
        echo "note: $dest is not on your PATH — add this to your shell profile:"
        echo "  export PATH=\"\$PATH:$dest\""
        ;;
esac
