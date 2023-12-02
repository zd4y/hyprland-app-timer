{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" ] (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        buildInputs = with pkgs; [ openssl ];
        nativeBuildInputs = with pkgs; [ pkg-config ];
      in
      {
        packages.default =
          let
            cargoToml = with builtins; (fromTOML (readFile ./Cargo.toml));
          in
          pkgs.rustPlatform.buildRustPackage {
            inherit nativeBuildInputs buildInputs;
            inherit (cargoToml.package) version;
            pname = cargoToml.package.name;
            src = pkgs.lib.cleanSource ./.;
            cargoLock.lockFile = ./Cargo.lock;
            meta.mainProgram = cargoToml.package.name;
          };

        devShells.default =
          pkgs.mkShell {
            inherit nativeBuildInputs;

            buildInputs = buildInputs ++ (with pkgs; [
              sqlx-cli
              rustc
              cargo
              clippy
              rustfmt
            ]);
          };
      });
}
