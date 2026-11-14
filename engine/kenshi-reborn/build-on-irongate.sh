#!/bin/bash
# KenshiReborn — Build on ironGate (.237)
# The Linux Forge: RTX 5070 + i9-14900K + Pop!_OS
#
# SSH to ironGate and run this:
#   ssh irongate 'bash -s' < build-on-irongate.sh
#
# Or from any LAN host:
#   sshpass -p irongate ssh root@192.168.4.237 'bash -s' < build-on-irongate.sh

set -e

echo "◆ KenshiReborn — ironGate Linux Build"
echo "  Host: $(hostname)"
echo "  GPU: $(nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null || echo 'no nvidia-smi')"
echo "  CPU: $(nproc) cores"
echo ""

# Install Rust if missing
if ! command -v cargo &> /dev/null; then
    echo "◆ Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

echo "  Rust: $(rustc --version)"
echo "  Cargo: $(cargo --version)"
echo ""

# Clone or pull
FORGEJO="http://gaia:GaiaForge2026!@192.168.4.244:3000"
BUILD_DIR="/opt/ludoSpring"

if [ -d "$BUILD_DIR" ]; then
    echo "◆ Pulling latest..."
    cd "$BUILD_DIR"
    GIT_MERGE_AUTOEDIT=no git pull origin main --no-rebase --no-edit
else
    echo "◆ Cloning from Forgejo..."
    git clone "${FORGEJO}/syntheticChemistry/ludoSpring.git" "$BUILD_DIR"
    cd "$BUILD_DIR"
fi

# Build
echo "◆ Building KenshiReborn (release)..."
cd engine/kenshi-reborn
cargo build --release 2>&1

BINARY="target/release/kenshi-reborn"
echo ""
echo "  ✓ Built: $(ls -lh $BINARY)"
echo ""

# Run flow analysis on ironGate's RTX 5070
echo "◆ Running PID flow analysis on ironGate..."
./$BINARY --flow 2>&1

echo ""
echo "◆ Binary ready at: ${BUILD_DIR}/engine/kenshi-reborn/${BINARY}"
echo "◆ To deploy to deck: scp ${BUILD_DIR}/engine/kenshi-reborn/${BINARY} deck@192.168.4.176:/home/deck/"
