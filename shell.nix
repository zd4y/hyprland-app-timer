{ pkgs ? import (fetchTarball "https://github.com/NixOS/nixpkgs/archive/nixos-22.11.tar.gz") {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    gcc
    pkg-config
    zlib
    openssl
    gcc
    xorg.libxcb
    sqlx-cli
  ];

  DATABASE_URL = "sqlite:" + builtins.getEnv "HOME" + "/app_timer2.db";
  LOG_FILE = builtins.getEnv "HOME" + "/app-timer2.log";
  SERVER_URL_FILE = builtins.getEnv "HOME" + "/app-timer2-server.txt";
}
