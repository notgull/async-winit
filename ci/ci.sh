#!/bin/sh

set -eu

# Run CI-based tests for async-winit

rx() {
  cmd="$1"
  shift

  (
    set -x
    "$cmd" "$@"
  )
}

aw_check_target() {
  target="$1"
  command="$2"

  echo ">> Check for $target using $command"
  rustup target add "$target"
  rx cargo "$command" --target "$target" --features android-native-activity
  cargo clean
}

aw_test_version() {
  version="$1"
  extended_tests="$2"

  rustup toolchain add "$version" --profile minimal
  rustup default "$version"

  echo ">> Testing various feature sets..."
  rx cargo test
  rx cargo build --all --all-features --all-targets
  rx cargo build --no-default-features --features x11
  rx cargo build --no-default-features --features wayland,wayland-dlopen
  cargo clean

  if ! $extended_tests; then
    return
  fi
  
  aw_check_target wasm32-unknown-unknown build
  aw_check_target x86_64-pc-windows-gnu build
  aw_check_target x86_64-apple-darwin check
  aw_check_target i686-unknown-linux-gnu build
  aw_check_target i686-pc-windows-gnu build
  aw_check_target aarch64-linux-android check
  aw_check_target x86_64-apple-ios check
  aw_check_target aarch64-apple-ios check
  aw_check_target x86_64-unknown-redox check
}

aw_tidy() {
  rustup toolchain add stable --profile minimal
  rustup default stable

  rx cargo fmt --all --check
  rx cargo clippy --all-features --all-targets
}

. "$HOME/.cargo/env"

aw_tidy
aw_test_version stable true
aw_test_version beta true
aw_test_version nightly true
aw_test_version 1.67.1 true

