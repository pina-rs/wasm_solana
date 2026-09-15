{
  pkgs,
  lib,
  config,
  inputs,
  ...
}:

let
  llvm = pkgs.llvmPackages_19;
  custom = inputs.ifiokjr-nixpkgs.packages.${pkgs.stdenv.hostPlatform.system};
in

{
  packages =
    with pkgs;
    [
      binaryen
      (inputs.nixpkgs-current.legacyPackages.${pkgs.stdenv.hostPlatform.system}.cargo-audit)
      cargo-binstall
      cargo-deny
      cargo-nextest
      cargo-run-bin
      chromedriver
      cmake
      curl
      custom.agave
      custom.monochange
      dprint
      gcc
      git
      gitleaks
      libiconv
      llvm.bintools
      llvm.clang
      llvm.clang-tools
      llvm.libclang.lib
      llvm.lld
      llvm.llvm
      libusb1 # needed by libusb1-sys via trezor-client in solana-keypair
      mdbook
      ninja
      nixfmt-rfc-style
      openssl
      perl
      pkg-config
      protobuf # needed for `solana-test-validator` in tests
      rust-jemalloc-sys
      # Upstream rustup 1.28+ fails in nix builds: check suite is network-sensitive
      # and the install phase fails generating shell completions because the sandbox
      # creates an empty settings.toml missing the required `version` field.
      (rustup.overrideAttrs (old: {
        doCheck = false;
        preInstall = (old.preInstall or "") + ''
          export HOME="$(mktemp -d)"
          mkdir -p "$HOME/.rustup"
          echo 'version = "12"' > "$HOME/.rustup/settings.toml"
        '';
      }))
      shfmt
      zizmor
      zlib
      zstd
    ]
    ++ lib.optionals stdenv.isDarwin [
      coreutils
    ]
    ++ lib.optionals stdenv.isLinux [
      libgcc.lib
      udev
    ];

  env = {
    OPENSSL_NO_VENDOR = "1";
    LIBCLANG_PATH = "${llvm.libclang.lib}/lib";
    CC = "${llvm.clang}/bin/clang";
    CXX = "${llvm.clang}/bin/clang++";
    PROTOC = "${pkgs.protobuf}/bin/protoc";
    LD_LIBRARY_PATH = "${config.env.DEVENV_PROFILE}/lib";
    WASM_BINDGEN_TEST_WEBDRIVER_JSON = "${config.env.DEVENV_ROOT}/setup/webdriver.json";
  }
  # cc-rs compiles vendored C/C++ (e.g. rocksdb inside the validator stack) by
  # passing `--target=arm64-apple-macosx` to `clang++`. The nix cc-wrapper
  # explicitly does not support that override ("multi-target compilers"
  # warning) and its mishandled include paths then fail to resolve libc++
  # headers against the Xcode sysroot (`unknown type name 'uint8_t'`). Hand
  # cc-rs Apple's toolchain on macOS instead; it pairs with the global Xcode
  # SDK, matching `apple.sdk = null` above.
  #
  # HOST_* is required because the nix stdenv setup hooks re-export
  # CC=clang/CXX=clang++ after `env`, silently overriding shell-level CC/CXX
  # values; cc-rs prefers HOST_* over CC/CXX, and nothing overwrites those.
  // lib.optionalAttrs pkgs.stdenv.hostPlatform.isDarwin {
    HOST_CC = "/usr/bin/clang";
    HOST_CXX = "/usr/bin/clang++";
    # macOS dyld ignores LIBCLANG_PATH for linked dylibs; the fallback path is
    # what bindgen's linked libclang resolves through.
    DYLD_FALLBACK_LIBRARY_PATH = "${llvm.libclang.lib}/lib";
  };

  # Rely on the global sdk for now as the nix apple sdk is not working for me.
  apple.sdk = null;

  # Use the stdenv conditionally.
  stdenv = pkgs.stdenv;

  git-hooks = {
    package = pkgs.prek;
    hooks = {
      "secrets:commit" = {
        enable = true;
        verbose = true;
        pass_filenames = false;
        name = "secrets";
        description = "Scan staged changes for leaked secrets with gitleaks.";
        entry = "${pkgs.gitleaks}/bin/gitleaks protect --staged --verbose --redact";
        stages = [ "pre-commit" ];
      };
      dprint = {
        enable = true;
        verbose = true;
        pass_filenames = true;
        name = "dprint fmt";
        description = "Format changed files with dprint before commit.";
        entry = "${pkgs.dprint}/bin/dprint fmt --allow-no-files";
        stages = [ "pre-commit" ];
      };
      nixfmt-rfc-style = {
        enable = true;
        pass_filenames = true;
        name = "nixfmt";
        description = "Format changed nix files.";
        entry = "${pkgs.nixfmt-rfc-style}/bin/nixfmt";
        stages = [ "pre-commit" ];
      };
    };
  };

  enterShell = ''
    set -e
    # The nix-only PATH sanitization drops /usr/bin on macOS, but vendored
    # build scripts depend on system tools: libusb1-sys (trezor-client via
    # solana-keypair) invokes `sw_vers` and aborts when it is missing.
    export PATH="/usr/bin:/bin:/usr/sbin:/sbin:$PATH";
    # Install the nightly rustfmt used by dprint before the git hooks run,
    # so the hook runner never races a concurrent toolchain update. Without
    # --force the install is idempotent: rustup skips it when the toolchain
    # already exists, so an interrupted download can never leave a broken
    # toolchain (missing librustc_driver) behind on CI.
    rustup toolchain install nightly --profile minimal --component rustfmt --no-self-update
    export LDFLAGS="$NIX_LDFLAGS";
  '';

  # disable dotenv since it breaks the variable interpolation supported by `direnv`
  dotenv.disableHint = true;

  scripts = {
    "release:change" = {
      exec = ''
        set -euo pipefail
        monochange run change $@
      '';
      description = "Create a changeset for the next release. Pass --package <name> --reason <text> and --bump <patch|minor|major>.";
    };
    "release:local" = {
      exec = ''
        set -euo pipefail
        # Runs the same steps as the CI release-pr workflow: PrepareRelease,
        # format, CommitRelease and (unless --create-pr=false) OpenReleaseRequest.
        monochange run release $@
      '';
      description = "Run the release flow locally to prepare, commit and open the release pull request.";
    };
    "publish:local" = {
      exec = ''
        set -euo pipefail
        # Escape hatch when CI publishing fails: verifies publish readiness
        # from the release record, then publishes every package with the local
        # cargo credentials (CARGO_REGISTRY_TOKEN or ~/.cargo/credentials).
        monochange step publish-readiness --from HEAD --format json
        monochange step publish-packages --log-level info --all
      '';
      description = "Publish the prepared release locally from the current release commit.";
    };
    "wasm-bindgen-test-runner" = {
      exec = ''
        set -e
        cargo bin wasm-bindgen-test-runner $@
      '';
      description = "The `wasm-bindgen-test-runner` executable";
    };
    "generate:keypair" = {
      exec = ''
        set -e
        solana-keygen new -s -o $DEVENV_ROOT/$1.json --no-bip39-passphrase || true
      '';
      description = "Generate a local solana keypair. Must provide a name.";
    };
    "install:cargo:bin" = {
      exec = ''
        set -e
        cargo bin --install
      '';
      description = "Install cargo binaries locally.";
    };
    "update:deps" = {
      exec = ''
        set -e
        cargo update
        devenv update
      '';
      description = "Update dependencies.";
    };
    "build:all" = {
      exec = ''
        set -e
        if [ -z "$CI" ]; then
          echo "Building project locally"
          cargo build --all-features
        else
          echo "Building in CI"
          cargo build --all-features --locked
        fi
      '';
      description = "Build all crates with all features activated.";
    };
    "build:docs" = {
      exec = ''
        RUSTUP_TOOLCHAIN="nightly" RUSTDOCFLAGS="--cfg docsrs" cargo doc --workspace --exclude example_program --exclude test_utils_solana
      '';
      description = "Build documentation site.";
    };
    "test:all" = {
      exec = ''
        set -e
        cargo test_memory_wallet_ssr
        cargo test_memory_wallet_docs
        cargo test_wasm_client_solana_ssr
        cargo test_wasm_client_solana_docs
        cargo test_streams
        WASM_BINDGEN_TEST_TIMEOUT=90 test:validator
      '';
      description = "Run all tests across the crates";
    };
    "test:validator" = {
      exec = ''
        set -e
        validator:bg &

        function cleanup {
          validator:kill
          kill -9 $! 2> /dev/null
        }
        trap cleanup EXIT

        cargo bin wait-for-them -t 10000 127.0.0.1:8899
        sleep 5

        echo "running tests in chrome..."
        CHROMEDRIVER=$DEVENV_PROFILE/bin/chromedriver cargo test_wasm

        # echo "running tests in firefox..."
        # GECKODRIVER=$DEVENV_PROFILE/bin/geckodriver cargo test_wasm
      '';
      description = "Run tests with a validator in the background.";
    };
    "coverage:all" = {
      exec = ''
        set -e
        cargo coverage_memory_wallet_ssr
        cargo coverage_memory_wallet_docs
        cargo coverage_wasm_client_solana_ssr
        cargo coverage_wasm_client_solana_docs
        cargo coverage_streams
        cargo coverage_codecov_report
      '';
      description = "Run coverage across the crates";
    };
    "security:deny" = {
      exec = ''
        set -euo pipefail
        cargo-deny check --config "$DEVENV_ROOT/deny.toml" bans licenses sources
      '';
      description = "Run cargo-deny checks (bans, licenses, sources).";
    };
    "security:audit" = {
      exec = ''
        set -euo pipefail
        # The advisory DB lives inside the cached target dir; a stale or partial
        # clone from a cache restore makes cargo-audit refuse to (re)initialize
        # it. Remove it so the clone always starts fresh.
        rm -rf "$DEVENV_ROOT/target/advisory-db-audit"
        # Ignore validator-stack advisories: these crates only run inside the
        # host-side solana-test-validator harness and are never part of any
        # published client API surface.
        cargo-audit audit \
          --db "$DEVENV_ROOT/target/advisory-db-audit" \
          --url "https://github.com/RustSec/advisory-db.git" \
          --deny yanked \
          --ignore RUSTSEC-2022-0093 \
          --ignore RUSTSEC-2024-0344 \
          --ignore RUSTSEC-2024-0421 \
          --file "$DEVENV_ROOT/Cargo.lock"
      '';
      description = "Run RustSec advisory audit for Cargo.lock (ignores validator-stack advisories).";
    };
    "security:zizmor" = {
      exec = ''
        set -euo pipefail
        zizmor --no-online-audits --no-progress .github
      '';
      description = "Audit GitHub Actions workflows and composite actions with zizmor.";
    };
    "security:all" = {
      exec = ''
        set -e
        security:deny
        security:audit
        security:zizmor
      '';
      description = "Run all dependency and workflow security checks.";
    };
    "fix:all" = {
      exec = ''
        set -e
        fix:clippy
        fix:format
      '';
      description = "Fix all autofixable problems.";
    };
    "fix:format" = {
      exec = ''
        set -e
        dprint fmt --config "$DEVENV_ROOT/dprint.json"
      '';
      description = "Format files with dprint.";
    };
    "fix:clippy" = {
      exec = ''
        set -e
        cargo clippy --fix --allow-dirty --allow-staged --all-features
      '';
      description = "Fix clippy lints for rust.";
    };
    "lint:monochange" = {
      exec = ''
        set -euo pipefail
        monochange check
      '';
      description = "Validate monochange release metadata.";
    };
    "lint:all" = {
      exec = ''
        set -e
        lint:clippy
        lint:monochange
        lint:format
      '';
      description = "Run all checks.";
    };
    "lint:format" = {
      exec = ''
        set -e
        dprint check
      '';
      description = "Check that all files are formatted.";
    };
    "lint:clippy" = {
      exec = ''
        set -e
        cargo clippy --all-features
      '';
      description = "Check that all rust lints are passing.";
    };
    "validator:run" = {
      exec = ''
        set -e
        solana-test-validator --warp-slot 1000 --reset --quiet
      '';
      description = "Run the solana validator.";
    };
    "validator:bg" = {
      exec = ''
        set -e
        validator:kill
        validator:run
      '';
      description = "Run the solana validator in the background";
    };
    "validator:kill" = {
      exec = ''
        pids=$(lsof -i :8899 -t)

        if [ -n "$pids" ]; then
          kill $pids
          echo "Killed processes listening on port $port: $pids"
        else
          echo "No processes found listening on port $port"
        fi
      '';
      description = "Kill any running validator";
    };
    "setup:vscode" = {
      exec = ''
        set -e
        rm -rf .vscode
        cp -r $DEVENV_ROOT/setup/editors/vscode .vscode
      '';
      description = "Setup the environment for vscode.";
    };
    "setup:helix" = {
      exec = ''
        set -e
        rm -rf .helix
        cp -r $DEVENV_ROOT/setup/editors/helix .helix
      '';
      description = "Setup for the helix editor.";
    };
  };
}
