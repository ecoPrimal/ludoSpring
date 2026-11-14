#!/usr/bin/env python3
"""THE GPU COLISEUM — Hippodrome of the RTX 5090

Dozens of Gaia AI play small games simultaneously.
Monkeys at typewriters. Ad nauseam. The GPU arena.

ARCHITECTURE:
  - Launch N instances of small games (Balatro, Stardew, Noita, etc.)
  - Each instance gets its own AI "monkey" — an ollama model watching the screen
  - Monkeys send keyboard/mouse inputs via pyautogui or Win32 API
  - sourDough watches the GPU coliseum — profiles the aggregate load
  - Games compete for GPU resources in the arena
  - Over time: monkeys learn, best strategies survive
  - The RTX 5090 is the coliseum floor — 600W of computational sand

CAPACITY:
  RTX 5090: 600W TDP, 32GB VRAM, 21,760 CUDA cores
  Small games: 50-100W each, 200-500MB VRAM each
  = 5-8 concurrent instances before GPU even sweats
  = More limited by CPU (9950X3D: 32 threads) and RAM (96GB)

PHASE 1: Launch multiple Balatros (card game — perfect for AI)
PHASE 2: Mix games — Balatro + Stardew + Noita running together
PHASE 3: AI agents send inputs, evolve strategies
PHASE 4: Tournament mode — best monkey advances
"""

import json
import os
import sys
import time
import subprocess
import statistics
import ctypes
import urllib.request
from datetime import datetime
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
import threading

STEAM_APPS = Path(r"M:\SteamLibrary\steamapps\common")
PROFILE_DIR = Path(r"C:\Users\mokke\Desktop\ludoSpring\data\coliseum")
PROFILE_DIR.mkdir(parents=True, exist_ok=True)

# Games eligible for the coliseum (small footprint)
ARENA_GAMES = {
    "balatro": {
        "appid": "2379780",
        "exe": STEAM_APPS / "Balatro" / "Balatro.exe",
        "name": "Balatro",
        "est_power_w": 70,
        "est_vram_mb": 200,
        "type": "card_roguelike",
        "ai_friendly": True,  # Turn-based, discrete choices
    },
    "stardew": {
        "appid": "413150",
        "exe": STEAM_APPS / "Stardew Valley" / "Stardew Valley.exe",
        "name": "Stardew Valley",
        "est_power_w": 58,
        "est_vram_mb": 400,
        "type": "farming_sim",
        "ai_friendly": True,  # Repeatable loops
    },
    "noita": {
        "appid": "881100",
        "exe": STEAM_APPS / "Noita" / "noita.exe",
        "name": "Noita",
        "est_power_w": 73,
        "est_vram_mb": 300,
        "type": "physics_roguelike",
        "ai_friendly": True,  # Death = restart = infinite attempts
    },
    "dredge": {
        "appid": "1562430",
        "exe": STEAM_APPS / "DREDGE" / "DREDGE.exe",
        "name": "DREDGE",
        "est_power_w": 94,
        "est_vram_mb": 500,
        "type": "fishing_horror",
        "ai_friendly": False,  # Needs spatial reasoning
    },
    "climbing": {
        "appid": "1574580",
        "exe": STEAM_APPS / "A Difficult Game About Climbing" / "A Difficult Game About Climbing.exe",
        "name": "A Difficult Game About Climbing",
        "est_power_w": 60,
        "est_vram_mb": 200,
        "type": "physics_platformer",
        "ai_friendly": True,  # Physics + trial-and-error = perfect for monkeys
    },
}

# AI models available across the organism
AI_MONKEYS = [
    {"url": "http://192.168.4.237:11434", "gate": "ironGate", "model": "qwen2.5:7b", "speed": "medium"},
    {"url": "http://192.168.4.237:11434", "gate": "ironGate", "model": "phi3", "speed": "fast"},
    {"url": "http://192.168.4.237:11434", "gate": "ironGate", "model": "llama3.2:3b", "speed": "fast"},
    {"url": "http://192.168.4.237:11434", "gate": "ironGate", "model": "tinyllama", "speed": "fastest"},
    {"url": "http://192.168.4.244:11436", "gate": "eastGate", "model": "gaia", "speed": "medium"},
    {"url": "http://192.168.4.244:11436", "gate": "eastGate", "model": "gaia-fast", "speed": "fast"},
    {"url": "http://192.168.4.244:11436", "gate": "eastGate", "model": "qwen2.5:7b", "speed": "medium"},
    {"url": "http://192.168.4.244:11436", "gate": "eastGate", "model": "mistral:7b", "speed": "medium"},
]

def sample_gpu():
    """Get current GPU state."""
    try:
        result = subprocess.run(
            ["nvidia-smi",
             "--query-gpu=temperature.gpu,power.draw,utilization.gpu,memory.used,fan.speed",
             "--format=csv,noheader,nounits"],
            capture_output=True, text=True, timeout=5
        )
        parts = [p.strip() for p in result.stdout.strip().split(",")]
        return {
            "temp_c": int(parts[0]), "power_w": float(parts[1]),
            "gpu_pct": int(parts[2]), "vram_mb": int(parts[3]),
            "fan_pct": int(parts[4])
        }
    except Exception:
        return None

def estimate_capacity():
    """How many games can the coliseum hold right now?"""
    gpu = sample_gpu()
    if not gpu:
        return 0, {}
    
    available_w = 550 - gpu["power_w"]  # Leave 50W headroom
    available_vram = 30000 - gpu["vram_mb"]  # Leave 2GB headroom
    
    capacity = {}
    for key, game in ARENA_GAMES.items():
        by_power = max(0, int(available_w / game["est_power_w"]))
        by_vram = max(0, int(available_vram / game["est_vram_mb"]))
        capacity[key] = min(by_power, by_vram, 8)  # Cap at 8 per game
    
    return gpu, capacity

def launch_game(game_key, instance_id=0):
    """Launch one game instance."""
    game = ARENA_GAMES[game_key]
    exe = game["exe"]
    
    if not exe.exists():
        # Fall back to Steam launch
        subprocess.Popen(["cmd", "/c", f"start steam://rungameid/{game['appid']}"])
        return None
    
    # Direct launch for multi-instance
    env = os.environ.copy()
    proc = subprocess.Popen([str(exe)], env=env, cwd=str(exe.parent))
    return proc

def ask_monkey(monkey, game_name, gpu_state, round_num):
    """Ask one AI monkey what move to make."""
    prompt = f"""You are Monkey #{monkey['model']} in the GPU Coliseum.
You are playing {game_name} on an RTX 5090. Round {round_num}.
GPU state: {gpu_state['temp_c']}°C, {gpu_state['power_w']:.0f}W, {gpu_state['gpu_pct']}% GPU.

What would you do next in {game_name}? Give ONE specific action.
Be creative. Be chaotic. You're a monkey at a typewriter.
One sentence only."""

    body = json.dumps({
        "model": monkey["model"],
        "prompt": prompt,
        "stream": False,
        "options": {"num_predict": 100}
    }).encode()
    
    try:
        req = urllib.request.Request(
            f"{monkey['url']}/api/generate", data=body,
            headers={"Content-Type": "application/json"}
        )
        with urllib.request.urlopen(req, timeout=30) as resp:
            result = json.loads(resp.read())
            return result.get("response", "").strip().split("\n")[0][:200]
    except Exception as e:
        return f"(monkey sleeping: {e})"

def run_coliseum(games_to_launch, rounds=10, round_interval=30):
    """Run the GPU Coliseum — launch games, assign monkeys, watch the arena."""
    
    print("╔══════════════════════════════════════════════════════════════╗")
    print("║          THE GPU COLISEUM — RTX 5090 HIPPODROME            ║")
    print("║                                                            ║")
    print("║   Monkeys at typewriters. Ad nauseam. The arena opens.     ║")
    print("╚══════════════════════════════════════════════════════════════╝")
    print()
    
    # Check capacity
    gpu, capacity = estimate_capacity()
    print(f"  Arena state: {gpu['temp_c']}°C | {gpu['power_w']:.0f}W | {gpu['gpu_pct']}% GPU | {gpu['vram_mb']}MB VRAM")
    print(f"  Capacity: {capacity}")
    print()
    
    # Assign monkeys to games
    assignments = []
    monkey_idx = 0
    for game_key in games_to_launch:
        game = ARENA_GAMES.get(game_key)
        if not game:
            continue
        monkey = AI_MONKEYS[monkey_idx % len(AI_MONKEYS)]
        assignments.append({
            "game_key": game_key,
            "game_name": game["name"],
            "monkey": monkey,
            "monkey_id": monkey_idx
        })
        monkey_idx += 1
    
    print(f"  {len(assignments)} combatants entering the arena:")
    for a in assignments:
        print(f"    Monkey #{a['monkey_id']} ({a['monkey']['gate']}/{a['monkey']['model']}) → {a['game_name']}")
    print()
    
    # Launch games via Steam (one at a time to avoid conflicts)
    print("  Launching games...")
    for a in assignments:
        game = ARENA_GAMES[a["game_key"]]
        subprocess.Popen(["cmd", "/c", f"start steam://rungameid/{game['appid']}"])
        print(f"    Launched {a['game_name']} (AppID {game['appid']})")
        time.sleep(5)
    
    # Wait for games to load
    print(f"\n  Games loading... waiting 20 seconds...")
    time.sleep(20)
    
    # Run rounds
    session_log = []
    for round_num in range(1, rounds + 1):
        gpu = sample_gpu()
        if not gpu:
            continue
        
        t = datetime.now().strftime("%H:%M:%S")
        print(f"\n  ═══ ROUND {round_num}/{rounds} ({t}) ═══")
        print(f"  Arena: {gpu['temp_c']}°C | {gpu['power_w']:.0f}W | {gpu['gpu_pct']}% GPU | {gpu['vram_mb']}MB VRAM | Fan:{gpu['fan_pct']}%")
        
        # Each monkey speaks
        round_data = {"round": round_num, "ts": datetime.now().isoformat(), "gpu": gpu, "monkeys": []}
        
        with ThreadPoolExecutor(max_workers=len(assignments)) as pool:
            futures = {}
            for a in assignments:
                f = pool.submit(ask_monkey, a["monkey"], a["game_name"], gpu, round_num)
                futures[f] = a
            
            for f in futures:
                a = futures[f]
                try:
                    action = f.result(timeout=30)
                except Exception:
                    action = "(monkey timed out)"
                
                print(f"    🐒 {a['monkey']['gate']}/{a['monkey']['model']} → {a['game_name']}: {action[:120]}")
                round_data["monkeys"].append({
                    "gate": a["monkey"]["gate"],
                    "model": a["monkey"]["model"],
                    "game": a["game_name"],
                    "action": action
                })
        
        session_log.append(round_data)
        
        if round_num < rounds:
            time.sleep(round_interval)
    
    # Save session
    session = {
        "type": "coliseum",
        "ts": datetime.now().isoformat(),
        "games": [a["game_name"] for a in assignments],
        "monkeys": len(assignments),
        "rounds": rounds,
        "log": session_log
    }
    
    session_path = PROFILE_DIR / f"coliseum-{datetime.now().strftime('%Y%m%d-%H%M%S')}.json"
    with open(session_path, "w") as f:
        json.dump(session, f, indent=2)
    
    print(f"\n  ═══ COLISEUM SESSION COMPLETE ═══")
    print(f"  {len(assignments)} monkeys × {rounds} rounds = {len(assignments) * rounds} actions")
    print(f"  Saved: {session_path.name}")
    print(f"  The monkeys rest. The arena cools. 🐒🏛️")
    
    return session

def main():
    if len(sys.argv) < 2:
        print("Usage: python hippodrome.py <game1> [game2] [game3] ...")
        print("  Games: balatro, stardew, noita, dredge, climbing")
        print("  Example: python hippodrome.py balatro stardew noita")
        print()
        print("Options:")
        print("  --rounds N     Number of rounds (default 10)")
        print("  --interval N   Seconds between rounds (default 30)")
        return
    
    games = []
    rounds = 10
    interval = 30
    
    i = 1
    while i < len(sys.argv):
        if sys.argv[i] == "--rounds" and i + 1 < len(sys.argv):
            rounds = int(sys.argv[i + 1])
            i += 2
        elif sys.argv[i] == "--interval" and i + 1 < len(sys.argv):
            interval = int(sys.argv[i + 1])
            i += 2
        else:
            games.append(sys.argv[i])
            i += 1
    
    run_coliseum(games, rounds=rounds, round_interval=interval)

if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\n\n  The coliseum gates close. The monkeys scatter. 🐒")
