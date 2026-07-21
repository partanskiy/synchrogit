{
  description = "Daemon that keeps git repositories synced with their remotes";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
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
        in
        rec {
          synchrogit = pkgs.rustPlatform.buildRustPackage {
            pname = "synchrogit";
            version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version;
            src = self;
            cargoLock.lockFile = ./Cargo.lock;

            nativeBuildInputs = [ pkgs.lowdown ];
            nativeCheckInputs = [ pkgs.gitMinimal ];

            postBuild = ''
              lowdown -s -Tman docs/synchrogit.1.md -o synchrogit.1
            '';

            postInstall = ''
              install -Dm644 synchrogit.1 $out/share/man/man1/synchrogit.1
              install -Dm644 packaging/config.example.toml $out/share/doc/synchrogit/config.example.toml
            ''
            + pkgs.lib.optionalString pkgs.stdenv.isLinux ''
              install -Dm644 packaging/systemd/synchrogit.service $out/lib/systemd/user/synchrogit.service
            '';

            meta = {
              description = "Daemon that keeps git repositories synced with their remotes";
              homepage = "https://github.com/partanskiy/synchrogit";
              license = pkgs.lib.licenses.mit;
              mainProgram = "synchrogit";
            };
          };
          default = synchrogit;
        }
      );
    };
}
