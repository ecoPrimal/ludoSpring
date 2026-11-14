#!/bin/bash
# KenshiReborn — Deploy to Steam Deck (deckGate)
# The Valve Spillway: engine flows from forge → deck → world
#
# Run this FROM ironGate (.237) or any Linux box on the LAN:
#   bash deploy-to-deck.sh
#
# Prerequisites:
#   - deckGate (.176) must be powered on (USB-C passthrough)
#   - Rust toolchain on the build host
#   - Network access to Forgejo (.244:3000)

set -e

DECK_IP="192.168.4.176"
DECK_USER="deck"
DECK_PASS="deckgate"
FORGEJO="http://gaia:GaiaForge2026!@192.168.4.244:3000"
REPO="syntheticChemistry/ludoSpring"

echo "╔══════════════════════════════════════════════════╗"
echo "║  KenshiReborn → Steam Deck (deckGate) Deploy    ║"
echo "║  The Valve Spillway — flow to the world          ║"
echo "╚══════════════════════════════════════════════════╝"
echo ""

# Step 1: Pull latest from Forgejo
echo "◆ Step 1: Pull from Forgejo"
if [ -d "/tmp/ludoSpring" ]; then
    cd /tmp/ludoSpring
    GIT_MERGE_AUTOEDIT=no git pull origin main --no-rebase --no-edit
else
    git clone "${FORGEJO}/${REPO}.git" /tmp/ludoSpring
    cd /tmp/ludoSpring
fi
echo "  ✓ Code synced"

# Step 2: Build for Linux x86_64 (native on ironGate or deck)
echo "◆ Step 2: Build KenshiReborn (release)"
cd engine/kenshi-reborn
cargo build --release
BINARY="target/release/kenshi-reborn"
echo "  ✓ Built: $(ls -lh $BINARY | awk '{print $5}')"

# Step 3: Run scan to verify
echo "◆ Step 3: Verify (scan mode)"
./$BINARY --scan 2>&1 | head -20
echo ""

# Step 4: Deploy to Steam Deck
echo "◆ Step 4: Deploy to deckGate ($DECK_IP)"
if ping -c 1 -W 2 $DECK_IP > /dev/null 2>&1; then
    sshpass -p "$DECK_PASS" scp $BINARY ${DECK_USER}@${DECK_IP}:/home/deck/kenshi-reborn
    sshpass -p "$DECK_PASS" ssh ${DECK_USER}@${DECK_IP} "chmod +x /home/deck/kenshi-reborn"
    echo "  ✓ Deployed to /home/deck/kenshi-reborn"
    
    # Step 5: Add to Steam as non-Steam game
    echo "◆ Step 5: Steam integration"
    sshpass -p "$DECK_PASS" ssh ${DECK_USER}@${DECK_IP} "mkdir -p /home/deck/.local/share/kenshi-reborn"
    echo "  → Add as non-Steam game in Gaming Mode"
    echo "  → Target: /home/deck/kenshi-reborn"
    echo "  → Launch options: --render (windowed) or --flow (PID analysis)"
else
    echo "  ⚠ deckGate is ASLEEP — deploy binary to /tmp for manual transfer"
    cp $BINARY /tmp/kenshi-reborn-linux-x86_64
    echo "  ✓ Binary at: /tmp/kenshi-reborn-linux-x86_64"
    echo "  → Transfer manually when deck wakes: scp /tmp/kenshi-reborn-linux-x86_64 deck@${DECK_IP}:/home/deck/"
fi

echo ""
echo "╔══════════════════════════════════════════════════╗"
echo "║  Spillway: Forgejo → Build → Deck → Steam → 🌍  ║"
echo "╚══════════════════════════════════════════════════╝"
