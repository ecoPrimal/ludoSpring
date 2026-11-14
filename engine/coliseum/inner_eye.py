#!/usr/bin/env python3
"""THE INNER EYE — Watch inside the GPU. No desktop. Pure silicon vision.

nvidia-smi dmon = device monitoring (SM, memory, encoder, decoder, power, temp)
nvidia-smi pmon = process monitoring (per-process SM%, mem%)

The coliseum doesn't bubble up to the desktop.
We watch the gladiators from INSIDE the arena floor.
Each process is a gladiator. The SM utilization is their sword swing.
VRAM allocation is their armor weight. Power draw is their heartbeat.

This IS the Shadow Cast paradigm realized:
  The GPU radiates. We don't look at the light.
  We read the radiation directly from the source.
"""

import json
import sys
import subprocess
import time
import re
from datetime import datetime
from pathlib import Path
from collections import defaultdict

PROFILE_DIR = Path(r"C:\Users\mokke\Desktop\ludoSpring\data\coliseum")
PROFILE_DIR.mkdir(parents=True, exist_ok=True)

def get_gpu_deep():
    """Deep GPU introspection — everything nvidia-smi can tell us."""
    result = subprocess.run(
        ["nvidia-smi", "--query-gpu="
         "temperature.gpu,temperature.memory,"
         "power.draw,power.limit,"
         "utilization.gpu,utilization.memory,"
         "memory.used,memory.free,memory.total,"
         "clocks.current.graphics,clocks.current.memory,clocks.current.sm,"
         "fan.speed,pstate,"
         "clocks_event_reasons.hw_slowdown,clocks_event_reasons.sw_thermal_slowdown",
         "--format=csv,noheader,nounits"],
        capture_output=True, text=True, timeout=5
    )
    if result.returncode != 0:
        return None
    
    parts = [p.strip() for p in result.stdout.strip().split(",")]
    return {
        "temp_gpu_c": safe_int(parts[0]),
        "temp_mem_c": parts[1] if parts[1] != "[N/A]" else None,
        "power_w": safe_float(parts[2]),
        "power_limit_w": safe_float(parts[3]),
        "gpu_util_pct": safe_int(parts[4]),
        "mem_util_pct": safe_int(parts[5]),
        "vram_used_mb": safe_int(parts[6]),
        "vram_free_mb": safe_int(parts[7]),
        "vram_total_mb": safe_int(parts[8]),
        "clock_gpu_mhz": safe_int(parts[9]),
        "clock_mem_mhz": safe_int(parts[10]),
        "clock_sm_mhz": safe_int(parts[11]),
        "fan_pct": safe_int(parts[12]),
        "pstate": parts[13] if len(parts) > 13 else "?",
        "hw_slowdown": parts[14] if len(parts) > 14 else "?",
        "thermal_slowdown": parts[15] if len(parts) > 15 else "?",
    }

def safe_int(s):
    try: return int(s.strip())
    except: return 0

def safe_float(s):
    try: return float(s.strip())
    except: return 0.0

def get_gpu_processes():
    """Per-process GPU usage — who's fighting in the arena."""
    result = subprocess.run(
        ["nvidia-smi", "pmon", "-c", "1"],
        capture_output=True, text=True, timeout=10
    )
    
    processes = []
    for line in result.stdout.strip().split("\n"):
        if line.startswith("#") or not line.strip():
            continue
        parts = line.split()
        if len(parts) >= 8:
            pid = parts[1]
            ptype = parts[2]
            sm = parts[3]
            mem = parts[4]
            name = parts[-1] if len(parts) > 8 else "unknown"
            
            processes.append({
                "pid": pid,
                "type": ptype,
                "sm_pct": sm if sm != "-" else "0",
                "mem_pct": mem if mem != "-" else "0",
                "name": name
            })
    
    return processes

def get_dmon_snapshot():
    """Device monitor snapshot — SM, mem, enc, dec, power, temp in one shot."""
    result = subprocess.run(
        ["nvidia-smi", "dmon", "-c", "1", "-s", "pucvmet"],
        capture_output=True, text=True, timeout=10
    )
    
    for line in result.stdout.strip().split("\n"):
        if line.startswith("#") or not line.strip():
            continue
        parts = line.split()
        if len(parts) >= 15:
            return {
                "power_w": safe_int(parts[1]),
                "temp_gpu": safe_int(parts[2]),
                "sm_pct": safe_int(parts[4]),
                "mem_pct": safe_int(parts[5]),
                "enc_pct": safe_int(parts[6]),
                "dec_pct": safe_int(parts[7]),
                "jpg_pct": safe_int(parts[8]),
                "mem_clock": safe_int(parts[10]),
                "gpu_clock": safe_int(parts[11]),
                "vram_mb": safe_int(parts[14]),
            }
    return None

def identify_gladiators(processes):
    """Identify which processes are games vs system."""
    games = []
    system = []
    
    game_names = ["balatro", "stardew", "noita", "dredge", "stalker", "kenshi",
                  "climbing", "creeper", "cw4", "elin", "selaco", "exanima",
                  "zomboid", "satisfactory", "civ", "elden", "death", "wukong"]
    
    for p in processes:
        name_lower = p["name"].lower()
        is_game = any(g in name_lower for g in game_names)
        if is_game:
            games.append(p)
        else:
            system.append(p)
    
    return games, system

def render_arena(gpu, dmon, processes, tick):
    """Render the arena view — pure text, no GUI, watching inside the GPU."""
    t = datetime.now().strftime("%H:%M:%S")
    games, system = identify_gladiators(processes)
    
    # Calculate totals
    game_sm = sum(safe_int(g["sm_pct"]) for g in games)
    sys_sm = sum(safe_int(s["sm_pct"]) for s in system)
    
    power_pct = int(gpu["power_w"] / gpu["power_limit_w"] * 100) if gpu["power_limit_w"] > 0 else 0
    vram_pct = int(gpu["vram_used_mb"] / gpu["vram_total_mb"] * 100) if gpu["vram_total_mb"] > 0 else 0
    
    # Power bar
    power_bar = "▓" * (power_pct // 5) + "░" * (20 - power_pct // 5)
    vram_bar = "▓" * (vram_pct // 5) + "░" * (20 - vram_pct // 5)
    sm_bar = "▓" * (gpu["gpu_util_pct"] // 5) + "░" * (20 - gpu["gpu_util_pct"] // 5)
    
    print(f"\n  ═══ INNER EYE — TICK {tick} ({t}) ═══")
    print(f"  ┌─ SILICON ─────────────────────────────────────────┐")
    print(f"  │ SM  {sm_bar} {gpu['gpu_util_pct']:3d}% ({gpu['clock_sm_mhz']}MHz)     │")
    print(f"  │ PWR {power_bar} {power_pct:3d}% ({gpu['power_w']:.0f}W/{gpu['power_limit_w']:.0f}W) │")
    print(f"  │ RAM {vram_bar} {vram_pct:3d}% ({gpu['vram_used_mb']}MB/{gpu['vram_total_mb']}MB)│")
    print(f"  │ TMP {gpu['temp_gpu_c']}°C  FAN {gpu['fan_pct']}%  P-STATE {gpu['pstate']}          │")
    print(f"  └───────────────────────────────────────────────────┘")
    
    if dmon:
        print(f"  ┌─ PIPELINES ──────────────────────────────────────┐")
        print(f"  │ Compute (SM):  {dmon['sm_pct']:3d}%   Memory BW: {dmon['mem_pct']:3d}%          │")
        print(f"  │ Encoder:       {dmon['enc_pct']:3d}%   Decoder:   {dmon['dec_pct']:3d}%          │")
        print(f"  │ JPEG:          {dmon['jpg_pct']:3d}%                              │")
        print(f"  └───────────────────────────────────────────────────┘")
    
    if games:
        print(f"  ┌─ GLADIATORS ({len(games)} in arena) ──────────────────────┐")
        for g in games:
            sm = g['sm_pct']
            print(f"  │   {g['name'][:20]:<20s}  SM:{sm:>3s}%  MEM:{g['mem_pct']:>3s}%   │")
        print(f"  │   Total game SM: {game_sm}%                            │")
        print(f"  └───────────────────────────────────────────────────┘")
    else:
        print(f"  ┌─ ARENA EMPTY — no gladiators detected ───────────┐")
        print(f"  │   System processes using SM: {sys_sm}%                  │")
        print(f"  │   Waiting for games to enter...                   │")
        print(f"  └───────────────────────────────────────────────────┘")
    
    return {
        "tick": tick, "ts": datetime.now().isoformat(),
        "gpu": gpu, "dmon": dmon,
        "gladiators": len(games), "game_sm_total": game_sm,
        "games": [{"name": g["name"], "sm": g["sm_pct"], "mem": g["mem_pct"]} for g in games]
    }

def main():
    duration = int(sys.argv[1]) if len(sys.argv) > 1 else 120  # seconds
    interval = int(sys.argv[2]) if len(sys.argv) > 2 else 5    # seconds
    ticks = duration // interval
    
    print("╔══════════════════════════════════════════════════════╗")
    print("║     THE INNER EYE — Watching Inside the GPU         ║")
    print("║                                                     ║")
    print("║     No desktop. No display. Pure silicon vision.    ║")
    print("║     We read the radiation at the source.            ║")
    print("╚══════════════════════════════════════════════════════╝")
    print(f"  Duration: {duration}s | Interval: {interval}s | Ticks: {ticks}")
    
    log = []
    for tick in range(1, ticks + 1):
        gpu = get_gpu_deep()
        dmon = get_dmon_snapshot()
        procs = get_gpu_processes()
        
        if gpu:
            entry = render_arena(gpu, dmon, procs, tick)
            log.append(entry)
        
        if tick < ticks:
            time.sleep(interval)
    
    # Save
    session_path = PROFILE_DIR / f"inner-eye-{datetime.now().strftime('%Y%m%d-%H%M%S')}.json"
    with open(session_path, "w") as f:
        json.dump(log, f, indent=2)
    print(f"\n  Inner eye session saved: {session_path.name}")

if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\n  The inner eye closes. 👁️")
