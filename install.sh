#!/usr/bin/env bash
set -euo pipefail

REPO="dimitriskourg/brainrunner"
RAW_BASE="https://raw.githubusercontent.com/${REPO}/main"
BINARY_URL="https://github.com/${REPO}/releases/latest/download/brainrunner"

BIN_DIR="${BRAINRUNNER_BIN_DIR:-/usr/local/bin}"
SYSTEMD_DIR="${BRAINRUNNER_SYSTEMD_DIR:-/etc/systemd/system}"
CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
CONFIG_DIR="$CONFIG_HOME/brainrunner"
CONFIG_FILE="$CONFIG_DIR/config.toml"

check_deps() {
    if ! command -v gh >/dev/null 2>&1; then
        echo "Error: 'gh' (GitHub CLI) is not installed or not in PATH." >&2
        echo "Install it from: https://cli.github.com/" >&2
        exit 1
    fi
    if ! command -v claude >/dev/null 2>&1; then
        echo "Error: 'claude' (Claude Code CLI) is not installed or not in PATH." >&2
        echo "Install it from: https://claude.ai/code" >&2
        exit 1
    fi
}

prompt_repo() {
    printf 'Which local repo should brainrunner watch? (full path): ' >&2
    read -r REPO_PATH </dev/tty
    echo "$REPO_PATH"
}

write_config() {
    local repo_path="$1"
    mkdir -p "$CONFIG_DIR"
    cat > "$CONFIG_FILE" <<EOF
repo_path = "$repo_path"
poll_interval_secs = 30
max_iterations = 20
max_iteration_secs = 1800
worktree_base = "/tmp/brainrunner-worktrees"
EOF
}

install_binary() {
    local dest="$BIN_DIR/brainrunner"
    sudo curl -fsSL -o "$dest" "$BINARY_URL"
    sudo chmod +x "$dest"
}

install_service() {
    local unit_dest="$SYSTEMD_DIR/brainrunner.service"
    sudo curl -fsSL -o "$unit_dest" "$RAW_BASE/brainrunner.service"
    sudo systemctl daemon-reload
    sudo systemctl enable --now brainrunner
}

main() {
    check_deps

    local repo_path
    repo_path=$(prompt_repo)

    install_binary
    write_config "$repo_path"
    install_service

    echo "brainrunner is running. Watch logs with: journalctl -fu brainrunner"
}

main "$@"
