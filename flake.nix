{
  description = "ZIKARON Desk";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f (import nixpkgs {
        inherit system;
        overlays = [ rust-overlay.overlays.default ];
      }));
    in {
      packages = forAll (pkgs:
        let
          toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          rustPlatform = pkgs.makeRustPlatform { cargo = toolchain; rustc = toolchain; };
          runtimeLibs = with pkgs; [ libxkbcommon wayland libglvnd libx11 libxcursor libxrandr libxi ];
        in rec {
          zikaron = rustPlatform.buildRustPackage {
            pname = "zikaron";
            version = "0.1.5";
            src = self;
            cargoLock.lockFile = ./Cargo.lock;
            cargoBuildFlags = [ "-p" "app" "-p" "zikaron-cli" ];
            doCheck = false;
            nativeBuildInputs = [ pkgs.pkg-config pkgs.makeWrapper ];
            buildInputs = runtimeLibs;
            postInstall = ''
              # Keep only the two shipped binaries, named as the Linux packages name them.
              for b in $out/bin/*; do
                case "$(basename "$b")" in app|zikaron) ;; *) rm "$b" ;; esac
              done
              mv $out/bin/app $out/bin/zikaron-desk
              wrapProgram $out/bin/zikaron-desk \
                --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath runtimeLibs}

              install -Dm644 packaging/linux/zikaron-desk.desktop $out/share/applications/zikaron-desk.desktop
              for png in packaging/icon/hicolor/*.png; do
                size=$(basename "$png" .png)
                install -Dm644 "$png" $out/share/icons/hicolor/''${size}x''${size}/apps/zikaron-desk.png
              done
              install -Dm644 packaging/icon/zikaron.svg $out/share/icons/hicolor/scalable/apps/zikaron-desk.svg
            '';
            meta = {
              description = "ZIKARON Desk";
              license = nixpkgs.lib.licenses.mit;
              mainProgram = "zikaron-desk";
            };
          };
          default = zikaron;
        });
    };
}
