{ pkgs ? import (
  fetchTarball {
    url="https://github.com/NixOS/nixpkgs/archive/72da83d9515b43550436891f538ff41d68eecc7f.tar.gz";
    sha256="177sws22nqkvv8am76qmy9knham2adfh3gv7hrjf6492z1mvy02y";
  }
) {} }:
let
in pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
  ];
}
