#!/bin/sh
# install.sh - Zero-prereq installer for agent-greenroom
# Usage: curl -fsSL https://raw.githubusercontent.com/vladNed/agent-greenroom/main/scripts/install.sh | sh
#        curl ... | sh -s -- --tool=opencode   (claude|opencode|codex, default: prompted)

set -e

REPO="vladNed/agent-greenroom"
INSTALL_DIR="$HOME/.local/bin"
BINARY_NAME="grn"
BINARY_PATH="$INSTALL_DIR/$BINARY_NAME"
CLI_TOOL=""
ALL_RELEASES=""  # populated after GitHub API call

# ---- Cleanup ---------------------------------------------------------------
TMPBIN=""
cleanup() { [ -n "$TMPBIN" ] && rm -f "$TMPBIN"; }
trap cleanup EXIT INT TERM

# ---- Helpers ---------------------------------------------------------------
log()     { printf '\033[36m[install]\033[0m %s\n' "$1"; }
success() { printf '\033[32m[✓]\033[0m %s\n' "$1"; }
error()   { printf '\033[31m[ERROR]\033[0m %s\n' "$1" >&2; exit 1; }
command_exists() { command -v "$1" >/dev/null 2>&1; }

http_get() {
    if command_exists curl; then
        curl -fsSL "$1"
    elif command_exists wget; then
        wget -qO- "$1"
    else
        error "Neither curl nor wget found. Please install one."
    fi
}

http_download() {
    if command_exists curl; then
        curl -fsSL -o "$2" "$1"
    elif command_exists wget; then
        wget -qO "$2" "$1"
    else
        error "Neither curl nor wget found. Please install one."
    fi
}

# ---- JSON patch (jq preferred, node fallback — both ship with Claude Code) -
# Usage: patch_json_mcp <file> <outer_key> <inner_key> <value_json>
patch_json_mcp() {
    _file="$1" _outer="$2" _inner="$3" _val="$4"
    mkdir -p "$(dirname "$_file")"

    if command_exists jq; then
        if [ -f "$_file" ]; then
            jq --arg k "$_inner" --argjson v "$_val" \
                ".${_outer}[\$k] = \$v" "$_file" > "$_file.tmp" && mv "$_file.tmp" "$_file"
        else
            jq -n --arg k "$_inner" --argjson v "$_val" \
                "{\"${_outer}\": {(\$k): \$v}}" > "$_file"
        fi
    elif command_exists node; then
        MCP_FILE="$_file" MCP_OUTER="$_outer" MCP_INNER="$_inner" MCP_VAL="$_val" \
        node -e "
const fs=require('fs'),e=process.env;
let c={};
try{c=JSON.parse(fs.readFileSync(e.MCP_FILE,'utf8'))}catch(_){}
if(!c[e.MCP_OUTER])c[e.MCP_OUTER]={};
c[e.MCP_OUTER][e.MCP_INNER]=JSON.parse(e.MCP_VAL);
fs.writeFileSync(e.MCP_FILE,JSON.stringify(c,null,2)+'\n');
"
    else
        error "jq or node is required to update settings JSON. Please install jq."
    fi
}

# ---- Release asset URL lookup ----------------------------------------------
# Usage: asset_url "agent-greenroom.md"
# Searches ALL_RELEASES for a browser_download_url ending in /<filename>
asset_url() {
    printf '%s' "$ALL_RELEASES" \
        | grep -o '"browser_download_url":"[^"]*"' \
        | grep -F "/${1}\"" | head -1 | cut -d'"' -f4
}

# ---- Platform detection ----------------------------------------------------
detect_platform() {
    os=$(uname -s | tr '[:upper:]' '[:lower:]')
    arch=$(uname -m)
    case "$os" in
        linux*)  os_part="unknown-linux-gnu" ;;
        darwin*) os_part="apple-darwin" ;;
        *) error "Unsupported OS: $os. Only Linux and macOS are supported." ;;
    esac
    case "$arch" in
        x86_64)        arch_part="x86_64" ;;
        aarch64|arm64) arch_part="aarch64" ;;
        *) error "Unsupported architecture: $arch" ;;
    esac
    printf 'agent-greenroom-%s-%s' "$arch_part" "$os_part"
}

# ---- CLI tool choice -------------------------------------------------------
parse_args() {
    for arg in "$@"; do
        case "$arg" in --tool=*) CLI_TOOL="${arg#--tool=}" ;; esac
    done
}

prompt_cli_choice() {
    [ -n "$CLI_TOOL" ] && return

    printf '\n\033[1mWhich AI coding tool are you using?\033[0m\n'
    printf '  1) Claude Code  (default)\n'
    printf '  2) OpenCode\n'
    printf '  3) Codex\n'
    printf '  4) Skip skill installation\n'
    printf 'Choice [1]: '

    choice=""
    if [ -t 0 ]; then
        read -r choice || true
    elif [ -r /dev/tty ]; then
        read -r choice < /dev/tty || true
    else
        log "Non-interactive — defaulting to Claude Code"
    fi

    case "${choice:-1}" in
        1|"") CLI_TOOL="claude" ;;
        2)    CLI_TOOL="opencode" ;;
        3)    CLI_TOOL="codex" ;;
        4)    CLI_TOOL="skip" ;;
        *)    CLI_TOOL="claude"; log "Unknown choice, defaulting to Claude Code" ;;
    esac
}

# ---- Per-tool install ------------------------------------------------------
install_claude() {
    SKILLS_DIR="$HOME/.claude/skills"
    SETTINGS_PATH="$HOME/.claude/settings.json"

    log "Installing Claude Code skill..."
    SKILL_URL=$(asset_url "agent-greenroom.md")
    [ -z "$SKILL_URL" ] && error "Skill asset 'agent-greenroom.md' not found in release $TAG_NAME"
    mkdir -p "$SKILLS_DIR"
    http_download "$SKILL_URL" "$SKILLS_DIR/agent-greenroom.md"
    success "Skill installed → $SKILLS_DIR/agent-greenroom.md"

    log "Registering MCP server in Claude settings..."
    if ! grep -q '"agent-greenroom"' "$SETTINGS_PATH" 2>/dev/null; then
        patch_json_mcp "$SETTINGS_PATH" "mcpServers" "agent-greenroom" \
            '{"type":"http","url":"http://127.0.0.1:7878/mcp"}'
        success "MCP server registered → $SETTINGS_PATH"
    else
        success "MCP server already registered"
    fi
}

install_opencode() {
    OC_DIR="$HOME/.config/opencode"
    INSTRUCTIONS_DIR="$OC_DIR/instructions"
    CONFIG_PATH="$OC_DIR/config.json"

    log "Installing OpenCode instructions..."
    SKILL_URL=$(asset_url "agent-greenroom-opencode.md")
    [ -z "$SKILL_URL" ] && error "Skill asset 'agent-greenroom-opencode.md' not found in release $TAG_NAME"
    mkdir -p "$INSTRUCTIONS_DIR"
    http_download "$SKILL_URL" "$INSTRUCTIONS_DIR/agent-greenroom.md"
    success "Instructions installed → $INSTRUCTIONS_DIR/agent-greenroom.md"

    log "Registering MCP server in OpenCode config..."
    if ! grep -q '"agent-greenroom"' "$CONFIG_PATH" 2>/dev/null; then
        patch_json_mcp "$CONFIG_PATH" "mcp" "agent-greenroom" \
            '{"type":"remote","url":"http://127.0.0.1:7878/mcp"}'
        success "MCP server registered → $CONFIG_PATH"
    else
        success "MCP server already registered"
    fi
}

install_codex() {
    CODEX_DIR="$HOME/.codex"
    CONFIG_PATH="$CODEX_DIR/config.json"

    log "Installing Codex instructions..."
    SKILL_URL=$(asset_url "agent-greenroom-codex.md")
    [ -z "$SKILL_URL" ] && error "Skill asset 'agent-greenroom-codex.md' not found in release $TAG_NAME"
    mkdir -p "$CODEX_DIR"
    http_download "$SKILL_URL" "$CODEX_DIR/agent-greenroom.md"
    success "Instructions installed → $CODEX_DIR/agent-greenroom.md"

    log "Registering MCP server in Codex config..."
    if ! grep -q '"agent-greenroom"' "$CONFIG_PATH" 2>/dev/null; then
        patch_json_mcp "$CONFIG_PATH" "mcpServers" "agent-greenroom" \
            '{"url":"http://127.0.0.1:7878/mcp"}'
        success "MCP server registered → $CONFIG_PATH"
    else
        success "MCP server already registered"
    fi
}

# ---- Main ------------------------------------------------------------------
parse_args "$@"

ASSET_NAME=$(detect_platform)
log "Detected platform: $ASSET_NAME"
log "Will install binary as: $BINARY_NAME"

# Ask which AI tool to configure before any downloads
prompt_cli_choice

# Fetch latest release (first entry includes pre-releases)
log "Fetching latest release info from GitHub..."
ALL_RELEASES=$(http_get "https://api.github.com/repos/$REPO/releases")
TAG_NAME=$(printf '%s' "$ALL_RELEASES" \
    | grep -o '"tag_name":"[^"]*"' | head -1 | cut -d'"' -f4)
DOWNLOAD_URL=$(printf '%s' "$ALL_RELEASES" \
    | grep -o '"browser_download_url":"[^"]*"' | grep "$ASSET_NAME" | head -1 | cut -d'"' -f4)

[ -z "$TAG_NAME" ] && error "No releases found. Check https://github.com/$REPO/releases"
[ -z "$DOWNLOAD_URL" ] && \
    error "Asset '$ASSET_NAME' not in release $TAG_NAME. Check https://github.com/$REPO/releases"

log "Downloading $ASSET_NAME ($TAG_NAME)..."

# Download binary to temp file; trap cleans it on failure
mkdir -p "$INSTALL_DIR"
TMPBIN="$BINARY_PATH.tmp"
http_download "$DOWNLOAD_URL" "$TMPBIN"
mv "$TMPBIN" "$BINARY_PATH"
TMPBIN=""  # committed — disable cleanup
chmod +x "$BINARY_PATH"
success "Binary installed → $BINARY_PATH"

# Ensure ~/.local/bin is in PATH
for rc in "$HOME/.bashrc" "$HOME/.zshrc"; do
    if [ -f "$rc" ] && ! grep -q "$INSTALL_DIR" "$rc"; then
        printf '\nexport PATH="%s:$PATH"\n' "$INSTALL_DIR" >> "$rc"
        log "Added $INSTALL_DIR to PATH in $rc"
    fi
done

# Skill + MCP registration
case "$CLI_TOOL" in
    claude)   install_claude ;;
    opencode) install_opencode ;;
    codex)    install_codex ;;
    skip)     log "Skipping skill installation" ;;
esac

printf '\n'
success "Installation complete!"
printf '\n'
log "Next steps:"
log "  1. Start the server:  grn"
log "  2. Reload your shell: source ~/.bashrc  (or ~/.zshrc)"
log "  3. Your AI tool should now see the agent-greenroom MCP server"
printf '\n'
log "Binary:  $BINARY_PATH"
log "Version: $TAG_NAME"
log "Repo:    https://github.com/$REPO"
printf '\n'
