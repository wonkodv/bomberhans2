{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        devShells.default = pkgs.mkShell rec {
          nativeBuildInputs = with pkgs; [
            cargo-expand
            cargo-edit
            cargo
            rustc
            clang
            clippy
            cmake
            fontconfig
            gdb
            gnumake
            just
            inotify-tools # to detect changes in the exe and restart the server
            llvmPackages_latest.bintools
            pkg-config
            pre-commit
            rust-analyzer
            rustfmt
          ];

          buildInputs = with pkgs; [
            libx11
            libxrandr
            libxcursor
            libxi
            libxkbcommon
            libGL
            fontconfig
            wayland
          ];

          LD_LIBRARY_PATH = nixpkgs.lib.makeLibraryPath buildInputs;
        };
      }
    );
}
