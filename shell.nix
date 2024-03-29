{ pkgs ? import <nixpkgs> {}}: 
let
	unstable = import (fetchTarball https://channels.nixos.org/nixos-unstable/nixexprs.tar.xz){};
  list1 = with pkgs.buildPackages; [
    cargo
    unstable.rustc
  ];
in pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
    pkg-config
    cmake
    alsa-lib
    fontconfig
  ] 
  ++ list1; 
  buildInputs = with pkgs; [systemd];
  dbus=pkgs.dbus;
}
