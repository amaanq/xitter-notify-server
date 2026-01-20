{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      fenix,
    }:
    let
      forAllSystems = nixpkgs.lib.genAttrs [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          toolchain = fenix.packages.${system}.stable.toolchain;
        in
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "xitter-notify-server";
            version = "0.1.0";
            src = ./.;
            cargoLock = {
              lockFile = ./Cargo.lock;
              outputHashes = {
                "xitter-txid-0.1.0" = "sha256-vUxG5ibP3OKqGkvrf+gLCIBq1hDlejgaiIO6KfnzIEs=";
              };
            };
            nativeBuildInputs = [ toolchain ];
          };
        }
      );

      devShells = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          toolchain = fenix.packages.${system}.stable.toolchain;
        in
        {
          default = pkgs.mkShell {
            packages = [
              toolchain
              pkgs.rust-analyzer
            ];
          };
        }
      );

      nixosModules.default =
        {
          config,
          lib,
          pkgs,
          ...
        }:
        let
          cfg = config.services.xitter-notify-server;
        in
        {
          options.services.xitter-notify-server = {
            enable = lib.mkEnableOption "Xitter notification server";

            package = lib.mkOption {
              type = lib.types.package;
              default = self.packages.${pkgs.system}.default;
              description = "The xitter-notify-server package to use";
            };

            listenAddr = lib.mkOption {
              type = lib.types.str;
              default = "127.0.0.1:3000";
              description = "Address and port to listen on";
            };

            dbPath = lib.mkOption {
              type = lib.types.str;
              default = "/var/lib/xitter-notify-server/xitter-notify-server.db";
              description = "Path to the SQLite database";
            };

            pollInterval = lib.mkOption {
              type = lib.types.int;
              default = 15;
              description = "Polling interval in seconds";
            };

            maxConcurrent = lib.mkOption {
              type = lib.types.int;
              default = 50;
              description = "Maximum concurrent polling requests";
            };

            user = lib.mkOption {
              type = lib.types.str;
              default = "xitter-notify";
              description = "User to run the service as";
            };

            group = lib.mkOption {
              type = lib.types.str;
              default = "xitter-notify";
              description = "Group to run the service as";
            };
          };

          config = lib.mkIf cfg.enable {
            users.users.${cfg.user} = {
              isSystemUser = true;
              inherit (cfg) group;
              home = "/var/lib/xitter-notify-server";
              createHome = true;
            };

            users.groups.${cfg.group} = { };

            systemd.services.xitter-notify-server = {
              description = "Xitter Notification Server";
              wantedBy = [ "multi-user.target" ];
              after = [ "network.target" ];

              environment = {
                XITTER_NOTIFY_LISTEN_ADDR = cfg.listenAddr;
                XITTER_NOTIFY_DB_PATH = cfg.dbPath;
                XITTER_NOTIFY_POLL_INTERVAL = toString cfg.pollInterval;
                XITTER_NOTIFY_MAX_CONCURRENT = toString cfg.maxConcurrent;
              };

              serviceConfig = {
                Type = "simple";
                User = cfg.user;
                Group = cfg.group;
                ExecStart = "${cfg.package}/bin/xitter-notify-server";
                Restart = "on-failure";
                RestartSec = 5;

                StateDirectory = "xitter-notify-server";
                StateDirectoryMode = "0750";

                NoNewPrivileges = true;
                ProtectSystem = "strict";
                ProtectHome = true;
                PrivateTmp = true;
                PrivateDevices = true;
                ProtectKernelTunables = true;
                ProtectKernelModules = true;
                ProtectControlGroups = true;
                RestrictAddressFamilies = [
                  "AF_INET"
                  "AF_INET6"
                  "AF_UNIX"
                ];
                RestrictNamespaces = true;
                LockPersonality = true;
                MemoryDenyWriteExecute = true;
                RestrictRealtime = true;
                RestrictSUIDSGID = true;
                RemoveIPC = true;

                ReadWritePaths = [ "/var/lib/xitter-notify-server" ];
              };
            };
          };
        };
    };
}
