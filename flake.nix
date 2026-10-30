{
  description = "Little Wardrobe family clothing inventory and reproducible deployment";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs = { self, nixpkgs }: let
    systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
    eachSystem = nixpkgs.lib.genAttrs systems;
    forSystem = system: let
      pkgs = import nixpkgs { inherit system; };
      source = pkgs.lib.cleanSourceWith {
        src = ./.;
        filter = path: type: !(builtins.elem (baseNameOf path) [ "node_modules" "target" "dist" ".git" "data" "test-results" "mockups" ".tanstack" "result" "backend" "docs" "deploy" ".github" "README.md" "AGENTS.md" "flake.nix" "flake.lock" ]);
      };
      frontend = pkgs.buildNpmPackage {
        pname = "littlewardrobe-ui"; version = "0.1.0"; src = source;
        nodejs = pkgs.nodejs_26;
        # Read dependencies from package-lock.json so lockfile-only updates need no hash bump.
        npmDeps = pkgs.importNpmLock { npmRoot = source; };
        npmConfigHook = pkgs.importNpmLock.npmConfigHook;
        installPhase = ''mkdir -p $out/share/littlewardrobe; cp -r dist/client/. $out/share/littlewardrobe/'';
      };
      backend = pkgs.rustPlatform.buildRustPackage {
        pname = "littlewardrobe-api"; version = "0.1.0";
        src = pkgs.lib.cleanSourceWith {src = ./backend; filter = path: type: !(builtins.elem (baseNameOf path) ["target" "bindings"]);};
        cargoLock.lockFile = ./backend/Cargo.lock;
        # reqwest verifies TLS with the platform store, so tests need CA certificates.
        nativeCheckInputs = [ pkgs.cacert ];
      };
    in {inherit pkgs source frontend backend;};
  in {
    packages = eachSystem (system: let build = forSystem system; inherit (build) pkgs frontend backend; in {
      inherit frontend backend;
      default = backend;
    } // pkgs.lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
      container = pkgs.dockerTools.buildLayeredImage {
        name = "ghcr.io/fruitiex/littlewardrobe"; tag = "main";
        contents = [ backend frontend pkgs.cacert pkgs.tzdata pkgs.coreutils pkgs.gnutar ];
        extraCommands = ''mkdir -p data tmp; chmod 1777 tmp'';
        fakeRootCommands = ''chown 10001:10001 data'';
        config = {
          User = "10001:10001"; WorkingDir = "/";
          Entrypoint = [ "${backend}/bin/littlewardrobe-api" ];
          Env = [ "LITTLEWARDROBE_BIND=0.0.0.0:8080" "LITTLEWARDROBE_DATA_DIR=/data" "LITTLEWARDROBE_STATIC_DIR=${frontend}/share/littlewardrobe" "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt" "TZDIR=${pkgs.tzdata}/share/zoneinfo" ];
          ExposedPorts."8080/tcp" = {}; Volumes."/data" = {};
        };
      };
    });
    devShells = eachSystem (system: let pkgs = (forSystem system).pkgs; in {
      default = pkgs.mkShell {
        packages = with pkgs; [ nodejs_26 python3 cargo rustc rustfmt clippy pkg-config gcc git kustomize sops ]
          ++ lib.optionals stdenv.hostPlatform.isLinux [ chromium ];
        shellHook = ''
          ${nixpkgs.lib.optionalString (nixpkgs.lib.hasSuffix "linux" system) "export PLAYWRIGHT_CHROMIUM_EXECUTABLE=${pkgs.chromium}/bin/chromium"}
          echo "Little Wardrobe: npm run dev | cargo run --manifest-path backend/Cargo.toml"
        '';
      };
    });
  };
}
