{
  description = "Time-aware KubeCon + CloudNativeCon schedule TUI";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in {
      packages = forAllSystems (pkgs: {
        default = pkgs.rustPlatform.buildRustPackage {
          pname = "kubecon-tui";
          version = "0.1.0";
          src = self;
          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = [ pkgs.makeWrapper ];

          postInstall = ''
            install -Dm644 data/sessions.json $out/share/kubecon-tui/sessions.json
            wrapProgram $out/bin/kubecon-tui \
              --add-flags "--data $out/share/kubecon-tui/sessions.json"${pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isLinux " \\\n  --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.wl-clipboard pkgs.xclip pkgs.xdg-utils ]}"}
          '';

          meta = with pkgs.lib; {
            description = "Time-aware terminal schedule for KubeCon + CloudNativeCon North America";
            license = licenses.mit;
            mainProgram = "kubecon-tui";
          };
        };
      });

      apps = forAllSystems (pkgs: {
        default = {
          type = "app";
          program = "${self.packages.${pkgs.stdenv.hostPlatform.system}.default}/bin/kubecon-tui";
        };
      });

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
            rust-analyzer
            python3
          ];
        };
      });
    };
}
