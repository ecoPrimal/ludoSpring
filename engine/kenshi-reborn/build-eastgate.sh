#!/bin/bash
# KenshiReborn — eastGate build script
# Run on eastGate (192.168.4.244) — RTX 4070 + Vulkan
# Pull from Forgejo, build, serve meshes from northGate HTTP

set -e

echo "╔══════════════════════════════════════════════════════╗"
echo "║  KenshiReborn — eastGate Build (RTX 4070)           ║"
echo "║  Building the second eye of the LAN MMO             ║"
echo "╚══════════════════════════════════════════════════════╝"

REPO_DIR="$HOME/ludoSpring"
FORGEJO="http://192.168.4.244:3000"

# Clone or pull
if [ -d "$REPO_DIR" ]; then
    echo "◆ Pulling latest..."
    cd "$REPO_DIR"
    GIT_MERGE_AUTOEDIT=no git pull origin main --no-rebase --no-edit
else
    echo "◆ Cloning from Forgejo..."
    cd "$HOME"
    git clone "$FORGEJO/syntheticChemistry/ludoSpring.git"
    cd "$REPO_DIR"
fi

# Check Rust
echo "◆ Rust version:"
rustc --version || { echo "ERROR: rustc not found. Install with: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"; exit 1; }
cargo --version

# Check GPU
echo "◆ GPU:"
nvidia-smi --query-gpu=name,memory.total,driver_version --format=csv,noheader 2>/dev/null || echo "No NVIDIA GPU (will use software renderer)"

# Build
echo "◆ Building kenshi-reborn..."
cd engine/kenshi-reborn
cargo build --release 2>&1

echo ""
echo "◆ BUILD COMPLETE"
echo "  Binary: $REPO_DIR/engine/kenshi-reborn/target/release/kenshi-reborn"
echo ""
echo "  To run bestiary (needs mesh files from northGate):"
echo "    # Copy meshes from northGate:"
echo "    scp northgate@192.168.4.147:'M:/SteamLibrary/steamapps/common/Kenshi/data/animal/meshes/*.mesh' /tmp/kenshi-meshes/"
echo "    # Or serve them via HTTP from northGate:8888"
echo "    ./target/release/kenshi-reborn --bestiary /tmp/kenshi-meshes/"
echo ""
echo "  To run single mesh:"
echo "    ./target/release/kenshi-reborn --mesh /tmp/kenshi-meshes/bull.mesh"
echo ""
echo "  To serve the globe overlay:"
echo "    cd web && python3 -m http.server 8888 --bind 0.0.0.0"
echo "    # http://192.168.4.244:8888/kenshi-globe.html"
