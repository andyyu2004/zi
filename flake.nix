{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      rust-overlay,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        grammar =
          {
            name,
            owner ? "tree-sitter",
            version,
            wasmHash,
            highlightsHash,
            highlightsPath ? "queries/highlights.scm",
          }:
          {
            inherit name;
            wasm = pkgs.fetchurl {
              url = "https://github.com/${owner}/tree-sitter-${name}/releases/download/${version}/tree-sitter-${name}.wasm";
              hash = wasmHash;
            };
            highlights = pkgs.fetchurl {
              url = "https://raw.githubusercontent.com/${owner}/tree-sitter-${name}/${version}/${highlightsPath}";
              hash = highlightsHash;
            };
          };

        nixGrammar = pkgs.pkgsCross.wasm32-wasip1.tree-sitter.buildGrammar {
          language = "nix";
          version = "0.3.0";
          src = pkgs.fetchFromGitHub {
            owner = "nix-community";
            repo = "tree-sitter-nix";
            rev = "v0.3.0";
            hash = "sha256:1aaq5yv1s8qq3abm0jm884hi4k6n2w0nzd1j3wizv1x1xk6qzlsl";
          };
        };

        grammars = pkgs.runCommand "zi-grammars" { } (
          pkgs.lib.concatMapStrings
            (g: ''
              mkdir -p $out/${g.name}
              cp ${g.wasm} $out/${g.name}/language.wasm
              cp ${g.highlights} $out/${g.name}/highlights.scm
            '')
            [
              (grammar {
                name = "rust";
                version = "v0.24.2";
                wasmHash = "sha256-JMib2SUiVeSuu8vX0tMIvZLIbdlaEw/cgO+klXe41zg=";
                highlightsHash = "sha256-DwNDEH8Up2kBV/UQkKl564+L/k6tp8YXY920xUsTEdE=";
              })
              (grammar {
                name = "go";
                version = "v0.23.4";
                wasmHash = "sha256-bfyOrNrQpU0MrQ2IiFG9Gc3RTYJYLxEPiIu/b55eLWQ=";
                highlightsHash = "sha256-gRgsmGVH66f6YxboLf1iH7E7j8ie+shUMq7lGkjtCJY=";
              })
              (grammar {
                name = "json";
                version = "v0.24.8";
                wasmHash = "sha256-0hGfuY1ZEnGbE/lFhXT4YI0tKd++Rfa+H4YOof4qJAU=";
                highlightsHash = "sha256-BRFSRGW1au0SJYB5IlTmi2q7v95xGfHQKxNay+J4Iz8=";
              })
              (grammar {
                name = "c";
                version = "v0.23.5";
                wasmHash = "sha256-bnLkYg9yn2cbMn8E2G5CRWRGg58J4Z4ubEcHj2gwz/I=";
                highlightsHash = "sha256-M3jYVN2mlbKygrlGgkdSTtQnHvdK9pEDPS42iDN5QJw=";
              })
              (grammar {
                name = "python";
                version = "v0.25.0";
                wasmHash = "sha256-FhCLUN9O6aMBaHlCUqtV58k7/Fdl1/oKo+M1dSxRX0c=";
                highlightsHash = "sha256-pnCPIJOBYY4rOYlyyPHM2JLwwGTqs1oqP5EcPiLnmn4=";
              })
              (grammar {
                name = "bash";
                version = "v0.25.1";
                wasmHash = "sha256-gpKRnIig99P7MdDNAlPKWpUxvB7egrBTfyxj3Yq+ano=";
                highlightsHash = "sha256-t0Ig2VT0hbdibSsrYfN7Ui4S6xgwgD44jlfdeX3JnxE=";
              })
              (grammar {
                name = "css";
                version = "v0.25.0";
                wasmHash = "sha256-iiOXf+JxNXzObyVO+Iyb6/OFRgLYBGYFrvakXAITXFk=";
                highlightsHash = "sha256-I/eUijgXoNBtcSAVjDvuTsW1jap35SSYWkc4s2TbF7I=";
              })
              (grammar {
                name = "html";
                version = "v0.23.2";
                wasmHash = "sha256-xI/Ngsfqi/lDGACIun8oxIsrtSh4dBeRaL+dMeOUz4U=";
                highlightsHash = "sha256-Hrs4EajNwFQ4W4R6Oqxvv3B5+u/Vs9+rW7rSVs1a/c8=";
              })
              (grammar {
                name = "yaml";
                owner = "tree-sitter-grammars";
                version = "v0.7.2";
                wasmHash = "sha256-Cg5evP6wsr8nLQcTlu7tEHssSyYXtp1ZWbE/eLNXpNY=";
                highlightsHash = "sha256-L7+71coJKNCn4J5Z+PKWSxSjKKfuDOeZJw2QE9zOSaA=";
              })
              (grammar {
                name = "toml";
                owner = "tree-sitter-grammars";
                version = "v0.7.0";
                wasmHash = "sha256-GsaoOCbDWmiFf4Ml4Ax49ry+9Otts5MabPMEH3bl4J8=";
                highlightsHash = "sha256-L7XGHTOnA4kxIlTJw5Lixd1jE9lY1uX490yt1eCBFwc=";
              })
              (grammar {
                name = "typescript";
                version = "v0.23.2";
                wasmHash = "sha256-d4Al21qL4OcPjMw2ceSG3+3dBIwl2einDCbeLhv2+X0=";
                highlightsHash = "sha256-4MNa24GRJ7/U+FP6xUGefYukR2AkYgHQSkpc4CKKEMU=";
              })
              (grammar {
                name = "markdown";
                owner = "tree-sitter-grammars";
                version = "v0.5.3";
                wasmHash = "sha256-3Z/BKsKATXx9p4fkd0Elsy5Ps8JE5ecDGnfLfdgDYCA=";
                highlightsHash = "sha256-LrBudmzNZy1JWZ0U85jwf09ffyIIJimT082SB8EHji8=";
                highlightsPath = "tree-sitter-markdown/queries/highlights.scm";
              })
              {
                name = "nix";
                wasm = "${nixGrammar}/parser.wasm";
                highlights = "${nixGrammar}/queries/highlights.scm";
              }
            ]
        );

        plugins = (pkgs.makeRustPlatform { cargo = rust; rustc = rust; }).buildRustPackage {
          pname = "zi-plugins";
          version = "0.1.0";
          src = source;
          cargoLock = {
            lockFile = ./Cargo.lock;
            outputHashes."crop-0.4.2" = "0mfs49xy1b2dg4kahw0j9lyp9ac6b969faa4r630ljvgl9p3k29h";
          };
          nativeBuildInputs = [ pkgs.cargo-component ];
          outputs = [ "out" "testPlugins" ];
          doCheck = false;
          dontStrip = true;

          buildPhase = ''
            runHook preBuild
            export CARGO_COMPONENT_CACHE_DIR="$TMPDIR/cargo-component-cache"
            cargo component build --locked --offline --release --target wasm32-unknown-unknown \
              --target-dir target -p plugin-example -p plugin-test
            runHook postBuild
          '';

          installPhase = ''
            runHook preInstall
            mkdir -p "$out" "$testPlugins"
            cp target/wasm32-unknown-unknown/release/plugin_example.wasm "$out/"
            cp target/wasm32-unknown-unknown/release/plugin_test.wasm "$testPlugins/"
            runHook postInstall
          '';
        };

        devShell = pkgs.mkShell {
          packages = [
            rust
            pkgs.cargo-nextest
            pkgs.cargo-component
            pkgs.neovim
            pkgs.gopls
            pkgs.go
            pkgs.tree-sitter
            pkgs.gnumake
            pkgs.just
          ];

          ZI_NIX_GRAMMAR_DIR = grammars;
          shellHook = ''
            export ZI_GRAMMAR_DIR="''${ZI_GRAMMAR_DIR:-$ZI_NIX_GRAMMAR_DIR}"
          '';
        };

        fs = pkgs.lib.fileset;
        # What CI's jobs read: the tracked files but Markdown, which nothing builds from
        source = fs.toSource {
          root = ./.;
          fileset = fs.difference ./. (fs.fileFilter (f: f.hasExt "md") ./.);
        };

        # CI's jobs, by their ids in ci.yml
        ciJobs = [
          "test"
          "clippy"
        ];
      in
      {
        packages = {
          inherit grammars plugins;
          plugin-test = plugins.testPlugins;
        };

        devShells.default = devShell;

        # A store path per CI job, named for the source and the dev shell it runs in. The job
        # pushes it to the Nix cache once it passes, and is skipped when it's there.
        ci = pkgs.lib.genAttrs ciJobs (
          job:
          builtins.toFile "zi-ci-${job}" (
            builtins.unsafeDiscardStringContext "${source}\n${devShell.drvPath}\n"
          )
        );
      }
    );
}
