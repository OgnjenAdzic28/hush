#!/bin/sh
# Installs Hush from the latest GitHub release, then runs `hush setup`:
#
#   curl -fsSL https://raw.githubusercontent.com/OgnjenAdzic28/hush/main/install.sh | sh
#
# HUSH_INSTALL_DIR picks the folder (default ~/.local/bin). HUSH_DOWNLOAD_URL
# points at another build (hush-macos.tar.gz, hush-linux-x86_64.tar.gz or
# hush-linux-aarch64.tar.gz), with its .sha256 next to it.
set -eu

repo="OgnjenAdzic28/hush"
dir="${HUSH_INSTALL_DIR:-$HOME/.local/bin}"

fail() {
	echo "hush install: $1" >&2
	exit 1
}

case "$(uname -s)" in
Darwin)
	major="$(sw_vers -productVersion | cut -d. -f1)"
	[ "$major" -ge 15 ] || fail "Hush needs macOS 15 or later (this Mac has $(sw_vers -productVersion))"
	build="hush-macos"
	;;
Linux)
	case "$(uname -m)" in
	x86_64 | amd64) build="hush-linux-x86_64" ;;
	aarch64 | arm64) build="hush-linux-aarch64" ;;
	*) fail "there's no Hush build for $(uname -m) Linux yet" ;;
	esac
	;;
*) fail "Hush runs on macOS and Linux for now. Windows is next: https://github.com/$repo/issues/133" ;;
esac
url="${HUSH_DOWNLOAD_URL:-https://github.com/$repo/releases/latest/download/$build.tar.gz}"

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
if command -v sha256sum >/dev/null 2>&1; then
	actual="$(sha256sum "$tmp/hush.tar.gz" | cut -d' ' -f1)"
else
	actual="$(shasum -a 256 "$tmp/hush.tar.gz" | cut -d' ' -f1)"
fi
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
	*/bash)
		# Terminals on macOS start login shells; on Linux they read .bashrc.
		if [ "$build" = hush-macos ]; then rc="$HOME/.bash_profile"; else rc="$HOME/.bashrc"; fi
		;;
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
