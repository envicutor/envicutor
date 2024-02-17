{ pkgs ? import (fetchTarball "https://github.com/NixOS/nixpkgs/archive/4fddc9be4eaf195d631333908f2a454b03628ee5.tar.gz") {} }:
let
in pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
    python3
    rustc
    cargo
  ];
}
