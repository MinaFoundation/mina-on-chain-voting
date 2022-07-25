{
  description = "On Chain Signalling Deployment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/22.05";
    flake-utils.url = "github:numtide/flake-utils";
    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
    deploy-rs.url = "github:serokell/deploy-rs";
    mina.url = "github:MinaProtocol/mina";
  };

  outputs = { self, nixpkgs, flake-utils, flake-compat, deploy-rs, mina }: 
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        appDependencies = with pkgs; [
          geos gdal
          # postgres with postgis support
          (postgresql.withPackages (p: [ p.postgis ]))

          (haskellPackages.ghcWithPackages (self: with haskellPackages; [
            curl xml tar zlib fused-effects megaparsec bytestring directory tmp-postgres json process
          ]))
        ];
      in rec {

        apps = flake-utils.lib.flattenTree {
          clean-archive-backups = pkgs.writeShellApplication {
            name = "clean-archive-backups";
            runtimeInputs = appDependencies;
            text = "runghc ./Tools/cleanArchiveDump.hs";
          };

          download-archive-dump = pkgs.writeShellApplication {
            name = "download-archive-dump";
            runtimeInputs = appDependencies;
            text = "runghc ./Tools/downloadArchiveDump.hs";
          };

          run-temp-database = pkgs.writeShellApplication {
            name = "run-temp-database";
            runtimeInputs = appDependencies;
            text = "runghc ./Tools/runTempDatabase.hs";
          };
        };

        defaultApp = apps.run-temp-database;

        deploy.nodes.onChain-signalling = {
          hostname = "35.203.38.140";

          profiles = { };
        };

        checks = builtins.mapAttrs (system: 
          deployLib: deployLib.deployChecks self.deploy
        ) deploy-rs.lib;

        devShell = pkgs.mkShell {

          buildInputs = with pkgs; [
            haskell-language-server
            rnix-lsp nixpkgs-fmt
            geos
            gdal
            nixpkgs-fmt
            (python38.withPackages (ps: with ps; [ lxml pycurl certifi beautifulsoup4 ]))
            # postgres with postgis support
            (postgresql.withPackages (p: [ p.postgis ]))

            (haskellPackages.ghcWithPackages (self: with haskellPackages; [
              curl xml tar zlib fused-effects megaparsec bytestring directory tmp-postgres json process
            ]))
          ];

          shellHook = ''
            runghc download_archive_dump.hs
          '';
        };
      }
    );
}
