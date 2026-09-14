{ pkgs, config, ... }:

{
  # The toolchain is this file's, not the machine's: a native rustc and cargo
  # for whatever this computer is, pinned by the lockfile beside this file.
  # Every cargo command in AGENTS.md, README.md and .config/wt.toml runs
  # inside `devenv shell`, so a session and a hook build the same bytes with
  # the same compiler. (A Homebrew rust under Rosetta once made a two-minute
  # test tier take half an hour; this is what stops that recurring.)
  languages.rust.enable = true;

  packages = [
    pkgs.git
  ];

  # `devenv shell -- unit` — the first tier, the one a session runs while
  # building and before every commit (AGENTS.md, Gates).
  scripts.unit = {
    description = "The unit tier: cargo test --workspace --lib";
    exec = ''
      set -euo pipefail
      cd ${config.devenv.root}
      cargo test --workspace --lib "$@"
    '';
  };

  # `devenv shell -- integration` — the second tier, run before a branch
  # lands (wt hook pre-merge).
  scripts.integration = {
    description = "The integration tier: cargo test --workspace";
    exec = ''
      set -euo pipefail
      cd ${config.devenv.root}
      cargo test --workspace "$@"
    '';
  };

  # `devenv shell -- system` — the third tier, when the brief says so.
  scripts.system = {
    description = "The system tier: cargo test --workspace --features system-tests";
    exec = ''
      set -euo pipefail
      cd ${config.devenv.root}
      cargo test --workspace --features system-tests "$@"
    '';
  };
}
