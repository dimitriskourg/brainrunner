# Curl-installable rolling release for aarch64

brainrunner targets a single machine (a personal Raspberry Pi, aarch64). Distribution is a GitHub Actions workflow that fires on every push to `main`, builds a native binary using a `ubuntu-24.04-arm` runner, and overwrites a single `latest` GitHub Release. Installation is a one-liner: `curl .../install.sh | bash`.

Tagged versioning was rejected — since there is only one install target and no downstream consumers, version tags add ceremony with no benefit. A rolling `latest` release means the installed binary always matches `main`.

The install script:
1. Checks that `gh` and `claude` are in `PATH` (both require manual auth anyway — automating their install would give a broken setup)
2. Prompts for `repo_path` (the only required config field)
3. Downloads the binary from the `latest` release to `/usr/local/bin/brainrunner`
4. Writes `~/.config/brainrunner/config.toml`
5. Installs and `systemctl enable --now`s the systemd unit

## Consequences

The install URL is stable as long as the GitHub repo name doesn't change. If cross-platform support is ever needed, the rolling-release model still works — just add more release assets per platform.
