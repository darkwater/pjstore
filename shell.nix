let
  moz_overlay = import (builtins.fetchTarball https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz);
  nixpkgs = import <nixpkgs> { overlays = [ moz_overlay ]; };
in
  with nixpkgs;
  stdenv.mkDerivation {
    name = "pjstore";
    buildInputs = [
      sqlite
      (nixpkgs.rustChannelOf { date = "2020-04-29"; channel = "nightly"; }).rust
    ];
  }
