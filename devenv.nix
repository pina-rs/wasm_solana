{
  pkgs,
  lib,
  config,
  ...
}:
let
  llvm = pkgs.llvmPackages_19;
in

{
  packages =
    with pkgs;
    [
      binaryen
      cargo-release
      cargo-run-bin
      chromedriver
      curl
      cmake
      dprint
      eget
      gcc
      libiconv
      llvm.bintools
      llvm.clang
      llvm.clang-tools
      llvm.libclang.lib
      llvm.lld
      llvm.llvm
      llvm.mlir
      nixfmt-rfc-style
      openssl
      perl
      pkg-config
      protobuf # needed for `solana-test-validator` in tests
      rust-jemalloc-sys
      rustup
      shfmt
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
    EGET_CONFIG = "${config.env.DEVENV_ROOT}/.eget/.eget.toml";
    OPENSSL_NO_VENDOR = "1";
    LIBCLANG_PATH = "${llvm.libclang.lib}/lib";
    CC = "${llvm.clang}/bin/clang";
    CXX = "${llvm.clang}/bin/clang++";
    PROTOC = "${pkgs.protobuf}/bin/protoc";
    LD_LIBRARY_PATH = "${config.env.DEVENV_PROFILE}/lib";
    WASM_BINDGEN_TEST_WEBDRIVER_JSON = "${config.env.DEVENV_ROOT}/setup/webdriver.json";
  };

  # Rely on the global sdk for now as the nix apple sdk is not working for me.
  # apple.sdk = if pkgs.stdenv.isDarwin then pkgs.apple-sdk_15 else null;
  apple.sdk = null;

  # Use the stdenv conditionally.
  # stdenv = if pkgs.stdenv.isLinux then llvm.stdenv else pkgs.stdenv;
  stdenv = pkgs.stdenv;

  enterShell = ''
    set -e
    export PATH="$DEVENV_ROOT/.eget/bin:$PATH";
    export LDFLAGS="$NIX_LDFLAGS";
  '';

  # disable dotenv since it breaks the variable interpolation supported by `direnv`
  dotenv.disableHint = true;

  tasks = {
    "rustfmt:nightly" = {
      exec = ''
        rustup toolchain install nightly --component rustfmt --force
      '';
      before = [ "devenv:enterShell" ];
    };
  };

  scripts = {
    "knope" = {
      exec = ''
        set -e
        cargo bin knope $@
      '';
      description = "The `knope` executable";
      binary = "bash";
    };
    "wasm-bindgen-test-runner" = {
      exec = ''
        set -e
        cargo bin wasm-bindgen-test-runner $@
      '';
      description = "The `wasm-bindgen-test-runner` executable";
      binary = "bash";
    };
    "generate:keypair" = {
      exec = ''
        set -e
        solana-keygen new -s -o $DEVENV_ROOT/$1.json --no-bip39-passphrase || true
      '';
      description = "Generate a local solana keypair. Must provide a name.";
      binary = "bash";
    };
    "install:all" = {
      exec = ''
        set -e
        install:cargo:bin
        install:eget
      '';
      description = "Install all packages.";
      binary = "bash";
    };
    "install:eget" = {
      exec = ''
        HASH=$(nix hash path --base32 ./.eget/.eget.toml)
        echo "HASH: $HASH"
        if [ ! -f ./.eget/bin/hash ] || [ "$HASH" != "$(cat ./.eget/bin/hash)" ]; then
          echo "Updating eget binaries"
          eget -D --to "$DEVENV_ROOT/.eget/bin"
          echo "$HASH" > ./.eget/bin/hash
        else
          echo "eget binaries are up to date"
        fi
      '';
      description = "Install github binaries with eget.";
    };
    "install:cargo:bin" = {
      exec = ''
        set -e
        cargo bin --install
      '';
      description = "Install cargo binaries locally.";
      binary = "bash";
    };
    "update:deps" = {
      exec = ''
        set -e
        cargo update
        devenv update
      '';
      description = "Update dependencies.";
      binary = "bash";
    };
    "build:all" = {
      exec = ''
        set -e
        if [ -z "$CI" ]; then
          echo "Builing project locally"
          cargo build --all-features
        else
          echo "Building in CI"
          cargo build --all-features --locked
        fi
      '';
      description = "Build all crates with all features activated.";
      binary = "bash";
    };
    "build:docs" = {
      exec = ''
        RUSTUP_TOOLCHAIN="nightly" RUSTDOCFLAGS="--cfg docsrs" cargo doc --workspace --exclude example_program --exclude test_utils_solana
      '';
      description = "Build documentation site.";
      binary = "bash";
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
      binary = "bash";
    };
    "test:validator" = {
      exec = ''
        set -e
        validator:bg &
        pid=$!

        function cleanup {
          validator:kill
          kill -9 $pid
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
      binary = "bash";
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
      binary = "bash";
    };
    "fix:all" = {
      exec = ''
        set -e
        fix:clippy
        fix:format
      '';
      description = "Fix all autofixable problems.";
      binary = "bash";
    };
    "fix:format" = {
      exec = ''
        set -e
        dprint fmt --config "$DEVENV_ROOT/dprint.json"
      '';
      description = "Format files with dprint.";
      binary = "bash";
    };
    "fix:clippy" = {
      exec = ''
        set -e
        cargo clippy --fix --allow-dirty --allow-staged --all-features
      '';
      description = "Fix clippy lints for rust.";
      binary = "bash";
    };
    "lint:all" = {
      exec = ''
        set -e
        lint:clippy
        lint:format
      '';
      description = "Run all checks.";
      binary = "bash";
    };
    "lint:format" = {
      exec = ''
        set -e
        dprint check
      '';
      description = "Check that all files are formatted.";
      binary = "bash";
    };
    "lint:clippy" = {
      exec = ''
        set -e
        cargo clippy --all-features
      '';
      description = "Check that all rust lints are passing.";
      binary = "bash";
    };
    "validator:run" = {
      exec = ''
        set -e
        solana-test-validator --warp-slot 1000 --reset --quiet
      '';
      description = "Run the solana validator.";
      binary = "bash";
    };
    "validator:bg" = {
      exec = ''
        set -e
        validator:kill
        validator:run
      '';
      description = "Run the solana validator in the background";
      binary = "bash";
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
      binary = "bash";
    };
    "setup:vscode" = {
      exec = ''
        set -e
        rm -rf .vscode
        cp -r $DEVENV_ROOT/setup/editors/vscode .vscode
      '';
      description = "Setup the environment for vscode.";
      binary = "bash";
    };
    "setup:helix" = {
      exec = ''
        set -e
        rm -rf .helix
        cp -r $DEVENV_ROOT/setup/editors/helix .helix
      '';
      description = "Setup for the helix editor.";
      binary = "bash";
    };
  };
}
