#!/usr/bin/env bash
set -euo pipefail

SCRIPT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/install.sh"
PASS=0
FAIL=0

assert_eq() {
    local desc="$1" expected="$2" actual="$3"
    if [[ "$expected" == "$actual" ]]; then
        echo "  PASS: $desc"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $desc"
        echo "        expected: $expected"
        echo "        actual:   $actual"
        FAIL=$((FAIL + 1))
    fi
}

assert_contains() {
    local desc="$1" needle="$2" haystack="$3"
    if [[ "$haystack" == *"$needle"* ]]; then
        echo "  PASS: $desc"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $desc"
        echo "        expected to contain: $needle"
        echo "        actual: $haystack"
        FAIL=$((FAIL + 1))
    fi
}

assert_file_contains() {
    local desc="$1" needle="$2" file="$3"
    if grep -qF "$needle" "$file" 2>/dev/null; then
        echo "  PASS: $desc"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $desc"
        echo "        file '$file' does not contain: $needle"
        FAIL=$((FAIL + 1))
    fi
}

SYSTEM_PATH="/bin:/usr/bin"

# Run install.sh with a stubs-only PATH (clean env, no system tools).
# Used for dep-check tests where we want only the stubs.
run_script_clean() {
    local stubs_dir="$1" home_dir="$2"; shift 2
    local extra_env=("$@")
    env -i PATH="$stubs_dir" HOME="$home_dir" "${extra_env[@]}" /bin/bash "$SCRIPT" 2>&1
}

# Run install.sh with stubs prepended to the system PATH so cat/mkdir/etc. are available.
# Used for full-flow tests.
run_script_with_input() {
    local input="$1" stubs_dir="$2" home_dir="$3"; shift 3
    local extra_env=("$@")
    echo "$input" | env -i PATH="$stubs_dir:$SYSTEM_PATH" HOME="$home_dir" "${extra_env[@]}" /bin/bash "$SCRIPT" 2>&1
}

make_stub() {
    local dir="$1" name="$2"
    mkdir -p "$dir"
    printf '#!/bin/bash\nexit 0\n' > "$dir/$name"
    chmod +x "$dir/$name"
}

make_recording_stub() {
    local dir="$1" name="$2" record_file="$3"
    mkdir -p "$dir"
    cat > "$dir/$name" <<EOF
#!/bin/bash
echo "\$*" >> "$record_file"
exit 0
EOF
    chmod +x "$dir/$name"
}

make_curl_stub() {
    local dir="$1"
    mkdir -p "$dir"
    cat > "$dir/curl" <<'EOF'
#!/bin/bash
while [[ $# -gt 0 ]]; do
    case "$1" in
        -o) touch "$2"; shift 2;;
        *) shift;;
    esac
done
exit 0
EOF
    chmod +x "$dir/curl"
}

make_sudo_stub() {
    local dir="$1"
    mkdir -p "$dir"
    cat > "$dir/sudo" <<'EOF'
#!/bin/bash
"$@"
EOF
    chmod +x "$dir/sudo"
}

# ===== Test 1: missing gh exits 1 with error message =====
echo "Test 1: missing gh in PATH"
tmpdir=$(mktemp -d)
make_stub "$tmpdir/stubs" claude

set +e
output=$(run_script_clean "$tmpdir/stubs" "$HOME")
ec=$?
set -e

assert_eq "exits with code 1" 1 "$ec"
assert_contains "error mentions gh" "gh" "$output"
rm -rf "$tmpdir"

# ===== Test 2: missing claude exits 1 with error message =====
echo "Test 2: missing claude in PATH"
tmpdir=$(mktemp -d)
make_stub "$tmpdir/stubs" gh

set +e
output=$(run_script_clean "$tmpdir/stubs" "$HOME")
ec=$?
set -e

assert_eq "exits with code 1" 1 "$ec"
assert_contains "error mentions claude" "claude" "$output"
rm -rf "$tmpdir"

# ===== Test 3: config.toml written with repo_path and defaults =====
echo "Test 3: config.toml written with correct content"
tmpdir=$(mktemp -d)
config_home="$tmpdir/config"
bin_dir="$tmpdir/bin"
mkdir -p "$bin_dir"

make_stub "$tmpdir/stubs" gh
make_stub "$tmpdir/stubs" claude
make_stub "$tmpdir/stubs" systemctl
make_curl_stub "$tmpdir/stubs"
make_sudo_stub "$tmpdir/stubs"

set +e
run_script_with_input "/my/test/repo" "$tmpdir/stubs" "$HOME" \
    "XDG_CONFIG_HOME=$config_home" \
    "BRAINRUNNER_BIN_DIR=$bin_dir" >/dev/null 2>&1
ec=$?
set -e

config_file="$config_home/brainrunner/config.toml"
assert_file_contains "config has repo_path"          'repo_path = "/my/test/repo"'              "$config_file"
assert_file_contains "config has poll_interval_secs" 'poll_interval_secs = 30'                  "$config_file"
assert_file_contains "config has max_iterations"     'max_iterations = 20'                      "$config_file"
assert_file_contains "config has max_iteration_secs" 'max_iteration_secs = 1800'                "$config_file"
assert_file_contains "config has worktree_base"      'worktree_base = "/tmp/brainrunner-worktrees"' "$config_file"
rm -rf "$tmpdir"

# ===== Test 4: binary downloaded to bin dir and made executable =====
echo "Test 4: binary downloaded to bin dir"
tmpdir=$(mktemp -d)
config_home="$tmpdir/config"
bin_dir="$tmpdir/bin"
mkdir -p "$bin_dir"

make_stub "$tmpdir/stubs" gh
make_stub "$tmpdir/stubs" claude
make_stub "$tmpdir/stubs" systemctl
make_curl_stub "$tmpdir/stubs"
make_sudo_stub "$tmpdir/stubs"

set +e
run_script_with_input "/my/test/repo" "$tmpdir/stubs" "$HOME" \
    "XDG_CONFIG_HOME=$config_home" \
    "BRAINRUNNER_BIN_DIR=$bin_dir" >/dev/null 2>&1
set -e

assert_eq "binary file exists"    "0" "$([ -f "$bin_dir/brainrunner" ] && echo 0 || echo 1)"
assert_eq "binary is executable"  "0" "$([ -x "$bin_dir/brainrunner" ] && echo 0 || echo 1)"
rm -rf "$tmpdir"

# ===== Test 5: systemd unit installed and activated =====
echo "Test 5: systemd unit installed and enabled"
tmpdir=$(mktemp -d)
config_home="$tmpdir/config"
bin_dir="$tmpdir/bin"
systemd_dir="$tmpdir/systemd"
calls_file="$tmpdir/systemctl_calls"
mkdir -p "$bin_dir" "$systemd_dir"

make_stub "$tmpdir/stubs" gh
make_stub "$tmpdir/stubs" claude
make_curl_stub "$tmpdir/stubs"
make_sudo_stub "$tmpdir/stubs"
make_recording_stub "$tmpdir/stubs" systemctl "$calls_file"

set +e
run_script_with_input "/my/test/repo" "$tmpdir/stubs" "$HOME" \
    "XDG_CONFIG_HOME=$config_home" \
    "BRAINRUNNER_BIN_DIR=$bin_dir" \
    "BRAINRUNNER_SYSTEMD_DIR=$systemd_dir" >/dev/null 2>&1
set -e

assert_eq "systemd unit file exists" "0" "$([ -f "$systemd_dir/brainrunner.service" ] && echo 0 || echo 1)"
calls=$(cat "$calls_file" 2>/dev/null || echo "")
assert_contains "daemon-reload called" "daemon-reload" "$calls"
assert_contains "enable called"        "enable"        "$calls"
rm -rf "$tmpdir"

# ===== Summary =====
echo ""
echo "Results: $PASS passed, $FAIL failed"
[[ $FAIL -eq 0 ]]
