{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";

    flake-utils.url = "github:numtide/flake-utils";

    flake-compat.url = "https://flakehub.com/f/edolstra/flake-compat/1.tar.gz";
  };

  outputs = { self, nixpkgs, fenix, flake-utils, flake-compat }:
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" ] (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        toolchain = fenix.packages.${system}.stable.toolchain;
        buildInputs = with pkgs; [ openssl ];
        nativeBuildInputs = with pkgs; [ pkg-config ];
      in
      {
        packages.default =
          let
            cargoToml = with builtins; (fromTOML (readFile ./Cargo.toml));
          in
          (pkgs.makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          }).buildRustPackage {
            inherit nativeBuildInputs buildInputs;
            inherit (cargoToml.package) version;
            pname = cargoToml.package.name;
            src = nixpkgs.lib.cleanSource ./.;
            cargoLock.lockFile = ./Cargo.lock;
          };

        devShells.default =
          pkgs.mkShell {
            inherit nativeBuildInputs;

            buildInputs = buildInputs ++ (with pkgs; [
              sqlx-cli
              toolchain
            ]);
          };
      });
}
