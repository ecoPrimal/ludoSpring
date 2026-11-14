#!/usr/bin/env python3
"""GPU BUDGET SCHEDULER — The Coliseum's Resource Governor

The RTX 5090 has a budget. Gaia spends it on games + learning.
Thermal and power at 80% max = the HARD CEILING.

BUDGET MODES:
  80/20  — 80% games, 20% AI learning (play heavy)
  20/80  — 20% games, 80% AI learning (learn heavy)
  20/60/20 — 20% games, 60% AI, 20% monitoring/overhead

BUDGET CEILING (80% of max):
  Power:  480W  (80% of 600W TDP)
  Temp:   72°C  (80% of 90°C limit)
  VRAM:   26GB  (80% of 32GB)
  GPU:    80%   utilization ceiling

The scheduler:
  1. Reads current GPU state
  2. Calculates remaining budget
  3. Launches/kills game instances to stay in budget
  4. Allocates remaining budget to AI processing
  5. Reports the split every tick

Gaia plays 100 games at a time — if they fit in the budget.
"""

import json
import sys
import subprocess
import time
import statistics
from datetime import datetime
from pathlib import Path

PROFILE_DIR = Path(r"C:\Users\mokke\Desktop\ludoSpring\data\coliseum")
PROFILE_DIR.mkdir(parents=True, exist_ok=True)

# Hard ceilings (80% of physical max)
CEILING = {
    "power_w": 480,     # 80% of 600W
    "temp_c": 72,       # 80% of 90°C
    "vram_mb": 26000,   # 80% of 32GB
    "gpu_pct": 80,      # 80% utilization
}

# Per-game cost estimates (from our profiling!)
GAME_COSTS = {
    "balatro":   {"power_w": 15, "vram_mb": 200, "gpu_pct": 3, "appid": "2379780"},
    "stardew":   {"power_w": 20, "vram_mb": 400, "gpu_pct": 5, "appid": "413150"},
    "noita":     {"power_w": 18, "vram_mb": 300, "gpu_pct": 3, "appid": "881100"},
    "dredge":    {"power_w": 30, "vram_mb": 500, "gpu_pct": 5, "appid": "1562430"},
    "cw4":       {"power_w": 25, "vram_mb": 400, "gpu_pct": 5, "appid": "848480"},
    "climbing":  {"power_w": 15, "vram_mb": 200, "gpu_pct": 3, "appid": "1574580"},
    "moldrise":  {"power_w": 15, "vram_mb": 200, "gpu_pct": 3, "appid": "2291850"},
    "sailforth": {"power_w": 20, "vram_mb": 300, "gpu_pct": 4, "appid": "1515210"},
}

# System baseline (idle Windows + Steam + Cursor)
SYSTEM_BASELINE = {"power_w": 55, "vram_mb": 1800, "gpu_pct": 3}

BUDGET_MODES = {
    "80/20":    {"games": 0.80, "ai": 0.20, "overhead": 0.00},
    "20/80":    {"games": 0.20, "ai": 0.80, "overhead": 0.00},
    "20/60/20": {"games": 0.20, "ai": 0.60, "overhead": 0.20},
    "60/20/20": {"games": 0.60, "ai": 0.20, "overhead": 0.20},
    "auto":     {"games": 0.50, "ai": 0.40, "overhead": 0.10},
}

def sample_gpu():
    try:
        r = subprocess.run(
            ["nvidia-smi", "--query-gpu=temperature.gpu,power.draw,utilization.gpu,memory.used",
             "--format=csv,noheader,nounits"],
            capture_output=True, text=True, timeout=5
        )
        parts = [p.strip() for p in r.stdout.strip().split(",")]
        return {"temp_c": int(parts[0]), "power_w": float(parts[1]),
                "gpu_pct": int(parts[2]), "vram_mb": int(parts[3])}
    except:
        return None

def compute_budget(mode_name="auto"):
    """Compute available budget after system baseline."""
    gpu = sample_gpu()
    if not gpu:
        return None, None, None
    
    mode = BUDGET_MODES.get(mode_name, BUDGET_MODES["auto"])
    
    # Available = ceiling - current usage
    available = {
        "power_w": max(0, CEILING["power_w"] - gpu["power_w"]),
        "temp_headroom_c": max(0, CEILING["temp_c"] - gpu["temp_c"]),
        "vram_mb": max(0, CEILING["vram_mb"] - gpu["vram_mb"]),
        "gpu_pct": max(0, CEILING["gpu_pct"] - gpu["gpu_pct"]),
    }
    
    # Split available budget by mode
    game_budget = {
        "power_w": available["power_w"] * mode["games"],
        "vram_mb": available["vram_mb"] * mode["games"],
        "gpu_pct": available["gpu_pct"] * mode["games"],
    }
    
    ai_budget = {
        "power_w": available["power_w"] * mode["ai"],
        "vram_mb": available["vram_mb"] * mode["ai"],
        "gpu_pct": available["gpu_pct"] * mode["ai"],
    }
    
    return gpu, available, {"game": game_budget, "ai": ai_budget, "mode": mode_name}

def calculate_capacity(game_budget):
    """How many of each game can we fit in the game budget?"""
    capacity = {}
    for game, cost in GAME_COSTS.items():
        by_power = int(game_budget["power_w"] / max(cost["power_w"], 1))
        by_vram = int(game_budget["vram_mb"] / max(cost["vram_mb"], 1))
        by_gpu = int(game_budget["gpu_pct"] / max(cost["gpu_pct"], 1))
        capacity[game] = min(by_power, by_vram, by_gpu)
    return capacity

def plan_arena(capacity, strategy="diverse"):
    """Plan which games to launch. Strategy: diverse, focused, or random."""
    plan = []
    
    if strategy == "diverse":
        # One of each, round-robin until budget exhausted
        games = list(capacity.keys())
        idx = 0
        total_power = 0
        total_vram = 0
        while True:
            game = games[idx % len(games)]
            cost = GAME_COSTS[game]
            if capacity[game] <= 0:
                idx += 1
                if idx >= len(games) * 2:
                    break
                continue
            plan.append(game)
            capacity[game] -= 1
            total_power += cost["power_w"]
            total_vram += cost["vram_mb"]
            idx += 1
            if len(plan) >= 20:  # Hard cap
                break
    
    elif strategy == "focused":
        # All of the cheapest game (Balatro)
        cheapest = min(GAME_COSTS, key=lambda g: GAME_COSTS[g]["power_w"])
        count = capacity.get(cheapest, 0)
        plan = [cheapest] * min(count, 20)
    
    return plan

def render_budget(gpu, available, split, capacity, plan):
    """Render the budget dashboard."""
    t = datetime.now().strftime("%H:%M:%S")
    mode = split["mode"]
    gb = split["game"]
    ab = split["ai"]
    
    pwr_used_pct = int(gpu["power_w"] / CEILING["power_w"] * 100)
    vram_used_pct = int(gpu["vram_mb"] / CEILING["vram_mb"] * 100)
    
    print(f"\n╔══════════════════════════════════════════════════════════╗")
    print(f"║  GPU BUDGET SCHEDULER — {mode} MODE — {t}            ║")
    print(f"╠══════════════════════════════════════════════════════════╣")
    print(f"║                                                        ║")
    print(f"║  CEILING (80% max):                                    ║")
    print(f"║    Power: {CEILING['power_w']}W | Temp: {CEILING['temp_c']}°C | VRAM: {CEILING['vram_mb']//1024}GB | GPU: {CEILING['gpu_pct']}%     ║")
    print(f"║                                                        ║")
    print(f"║  CURRENT:                                              ║")
    print(f"║    Power: {gpu['power_w']:5.0f}W ({pwr_used_pct}% of ceiling)                  ║")
    print(f"║    Temp:  {gpu['temp_c']:5d}°C (headroom: {available['temp_headroom_c']}°C)               ║")
    print(f"║    VRAM:  {gpu['vram_mb']:5d}MB ({vram_used_pct}% of ceiling)                ║")
    print(f"║    GPU:   {gpu['gpu_pct']:5d}%                                       ║")
    print(f"║                                                        ║")
    print(f"║  GAME BUDGET ({mode.split('/')[0]}%):                                 ║")
    print(f"║    Power: {gb['power_w']:5.0f}W | VRAM: {gb['vram_mb']:5.0f}MB | GPU: {gb['gpu_pct']:4.0f}%    ║")
    print(f"║                                                        ║")
    print(f"║  AI BUDGET:                                            ║")
    print(f"║    Power: {ab['power_w']:5.0f}W | VRAM: {ab['vram_mb']:5.0f}MB | GPU: {ab['gpu_pct']:4.0f}%    ║")
    print(f"║                                                        ║")
    
    print(f"║  ARENA CAPACITY (games that fit in budget):            ║")
    for game, count in sorted(capacity.items(), key=lambda x: -x[1]):
        bar = "█" * min(count, 20) + "░" * max(0, 20 - count)
        print(f"║    {game[:10]:<10s} {bar} {count:3d}x    ║")
    
    total = sum(capacity.values())
    print(f"║                                                        ║")
    print(f"║  TOTAL CAPACITY: {total} simultaneous game instances      ║")
    
    if plan:
        print(f"║                                                        ║")
        print(f"║  PLANNED ARENA ({len(plan)} gladiators):                      ║")
        from collections import Counter
        for game, count in Counter(plan).items():
            print(f"║    {count}x {game}                                        ║"[:59] + "║")
    
    print(f"║                                                        ║")
    print(f"╚══════════════════════════════════════════════════════════╝")
    
    return {
        "ts": datetime.now().isoformat(),
        "mode": mode,
        "gpu": gpu,
        "available": available,
        "game_budget": gb,
        "ai_budget": ab,
        "capacity": capacity,
        "plan": plan,
        "total_capacity": total
    }

def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else "auto"
    strategy = sys.argv[2] if len(sys.argv) > 2 else "diverse"
    
    if mode not in BUDGET_MODES:
        print(f"Modes: {', '.join(BUDGET_MODES.keys())}")
        return
    
    print("╔══════════════════════════════════════════════════════════╗")
    print("║     GPU BUDGET SCHEDULER — THE COLISEUM GOVERNOR       ║")
    print("║                                                        ║")
    print("║  Thermal + Power at 80% max = HARD CEILING             ║")
    print("║  Gaia fills the arena until the budget says stop.      ║")
    print("╚══════════════════════════════════════════════════════════╝")
    
    gpu, available, split = compute_budget(mode)
    if not gpu:
        print("  GPU unreachable")
        return
    
    capacity = calculate_capacity(split["game"])
    plan = plan_arena(dict(capacity), strategy)
    
    result = render_budget(gpu, available, split, capacity, plan)
    
    # Save
    path = PROFILE_DIR / f"budget-{mode.replace('/','-')}-{datetime.now().strftime('%H%M%S')}.json"
    with open(path, "w") as f:
        json.dump(result, f, indent=2)
    print(f"\n  Budget plan saved: {path.name}")

if __name__ == "__main__":
    main()
