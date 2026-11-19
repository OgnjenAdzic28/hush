#!/bin/sh
# Installs Hush from the latest GitHub release, then runs `hush setup`:
#
#   curl -fsSL https://raw.githubusercontent.com/OgnjenAdzic28/hush/main/install.sh | sh
#
# HUSH_INSTALL_DIR picks the folder (default ~/.local/bin). HUSH_DOWNLOAD_URL
# points at another hush-macos.tar.gz, with its .sha256 next to it.
set -eu

repo="OgnjenAdzic28/hush"
dir="${HUSH_INSTALL_DIR:-$HOME/.local/bin}"
url="${HUSH_DOWNLOAD_URL:-https://github.com/$repo/releases/latest/download/hush-macos.tar.gz}"

fail() {
	echo "hush install: $1" >&2
	exit 1
}

[ "$(uname -s)" = Darwin ] ||
	fail "Hush runs on macOS for now. Windows and Linux are next: https://github.com/$repo/issues/30"
major="$(sw_vers -productVersion | cut -d. -f1)"
[ "$major" -ge 15 ] || fail "Hush needs macOS 15 or later (this Mac has $(sw_vers -productVersion))"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "Downloading Hush..."
fetch() {
	case "$1" in
	file://*) cp "${1#file://}" "$2" ;;
	*) curl -fsSL "$1" -o "$2" ;;
	esac
}
fetch "$url" "$tmp/hush.tar.gz" || fail "couldn't download $url"
fetch "$url.sha256" "$tmp/hush.sha256" || fail "couldn't download $url.sha256"
expected="$(cut -d' ' -f1 <"$tmp/hush.sha256")"
actual="$(shasum -a 256 "$tmp/hush.tar.gz" | cut -d' ' -f1)"
[ "$expected" = "$actual" ] || fail "the download doesn't match its checksum; try again"
tar -xzf "$tmp/hush.tar.gz" -C "$tmp"

mkdir -p "$dir"
# Replace by renaming, so a Hush host still running the old file keeps it;
# the next command notices the new build and starts a fresh host.
install -m 755 "$tmp/hush" "$dir/.hush.new"
mv -f "$dir/.hush.new" "$dir/hush"
echo "Installed $("$dir/hush" --version) to $dir/hush"

case ":$PATH:" in
*":$dir:"*) ;;
*)
	case "$dir" in
	"$HOME"/*) shown="\$HOME${dir#"$HOME"}" ;;
	*) shown="$dir" ;;
	esac
	line="export PATH=\"$shown:\$PATH\""
	case "${SHELL:-}" in
	*/zsh) rc="$HOME/.zshrc" ;;
	*/bash) rc="$HOME/.bash_profile" ;;
	*) rc="" ;;
	esac
	if [ -n "$rc" ]; then
		grep -qsF "$line" "$rc" || printf '\n# Hush\n%s\n' "$line" >>"$rc"
		echo "Added hush to your PATH in $rc. Open a new terminal, or run: $line"
	else
		echo "Add $dir to your PATH to run hush from anywhere."
	fi
	;;
esac

echo
"$dir/hush" setup || true
