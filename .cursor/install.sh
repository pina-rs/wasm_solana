#!/usr/bin/env bash
# Bootstrap Nix and devenv for a Cursor cloud agent.
#
# The VM has no systemd, so the multi-user Nix daemon cannot run. The
# official single-user installer is idempotent and does not edit shell
# rc files. Login shells skip ~/.bashrc, and non-login shells skip
# /etc/profile, so nix and devenv are symlinked into /usr/local/bin,
# which both kinds of shell already search.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

export DEBIAN_FRONTEND=noninteractive

if ! command -v curl >/dev/null 2>&1 || ! command -v sudo >/dev/null 2>&1; then
	echo "curl and sudo are required" >&2
	exit 1
fi

if [[ ! -x "${HOME}/.nix-profile/bin/nix" ]]; then
	curl -fsSL -o /tmp/nix-install https://nixos.org/nix/install
	sh /tmp/nix-install --no-daemon --yes --no-channel-add --no-modify-profile
	rm -f /tmp/nix-install
fi

mkdir -p "${HOME}/.config/nix"
nix_conf="${HOME}/.config/nix/nix.conf"
touch "${nix_conf}"

ensure_nix_line() {
	local key="$1"
	local line="$2"
	if grep -q "^${key} " "${nix_conf}"; then
		return 0
	fi
	printf '%s\n' "${line}" >>"${nix_conf}"
}

# Flakes are how devenv.lock is evaluated. The devenv cache is the same
# substituter CI configures, so the profile is downloaded rather than built.
ensure_nix_line experimental-features "experimental-features = nix-command flakes"
ensure_nix_line accept-flake-config "accept-flake-config = true"
ensure_nix_line extra-substituters "extra-substituters = https://devenv.cachix.org"
ensure_nix_line extra-trusted-public-keys "extra-trusted-public-keys = devenv.cachix.org-1:w1cLUi8dv3hnoSPGAuibQv+f9TZLr6cv/Hm9XgU50cw="

# Login shells. Non-login shells never read this file.
sudo tee /etc/profile.d/nix-devenv.sh >/dev/null <<'EOF'
if [ -f "${HOME}/.nix-profile/etc/profile.d/nix.sh" ]; then
	. "${HOME}/.nix-profile/etc/profile.d/nix.sh"
fi
EOF
sudo chmod 644 /etc/profile.d/nix-devenv.sh

# shellcheck disable=SC1091
. "${HOME}/.nix-profile/etc/profile.d/nix.sh"

link_profile_bins() {
	local bin name
	for bin in "${HOME}/.nix-profile/bin/"*; do
		[[ -e "${bin}" ]] || continue
		name="$(basename "${bin}")"
		sudo ln -sfn "${bin}" "/usr/local/bin/${name}"
	done
}

# Match the devenv revision pinned in devenv.lock, which is also what CI
# installs. `nixpkgs#devenv` would float away from that lock.
devenv_revision="$(
	nix eval --impure --raw --expr '
		let
			lock = builtins.fromJSON (builtins.readFile ./devenv.lock);
			revision = lock.nodes.devenv.locked.rev;
		in
			if builtins.match "^[0-9a-f]{40}$" revision != null then
				revision
			else
				throw "devenv.lock does not contain an immutable Devenv revision"
	'
)"

if ! nix profile list | grep -q "github:cachix/devenv/${devenv_revision}"; then
	nix profile install --accept-flake-config "github:cachix/devenv/${devenv_revision}#devenv"
	# shellcheck disable=SC1091
	. "${HOME}/.nix-profile/etc/profile.d/nix.sh"
fi

link_profile_bins

# Download and link the pinned profile (clang, protoc, agave, chromium,
# monochange, dprint, ...). A second run reuses the Nix store.
devenv shell -- true

# The repo's cargo aliases (`cargo nextest`, `cargo test_wasm`, ...) go
# through `cargo bin` and the versions pinned in Cargo.toml. enterShell
# also installs the nightly rustfmt that `dprint` invokes. Both are
# idempotent.
devenv shell -- install:cargo:bin
devenv shell -- cargo fetch --locked
