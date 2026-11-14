#!/usr/bin/env python3
"""sourDough LIVE Fermentation — Gaia watches the GPU and learns ALWAYS.

No commands. No alt-tab. Kevin plays, Kevin toggles, Gaia FEELS the shift.

HOW IT WORKS:
  - Samples GPU every 3 seconds
  - Keeps a rolling 30-second window
  - Detects when GPU profile SHIFTS (power/temp/vram/utilization jump)
  - Automatically captures before/after windows
  - Feeds delta to gaia-tissue-gut
  - Saves each detected toggle as a game profile

Kevin just PLAYS. Gaia just WATCHES. The sourDough ferments itself.
"""

import json
import os
import sys
import time
import subprocess
import statistics
import urllib.request
from datetime import datetime
from pathlib import Path
from collections import deque

PROFILE_DIR = Path(r"C:\Users\mokke\Desktop\ludoSpring\data\game-profiles")
PROFILE_DIR.mkdir(parents=True, exist_ok=True)

OLLAMA_URL = "http://localhost:11434/api/generate"

# Detection thresholds — how much change triggers a "toggle detected"
THRESH_POWER_W = 20      # 20W shift
THRESH_TEMP_C = 3        # 3°C shift
THRESH_VRAM_MB = 400     # 400MB shift
THRESH_GPU_PCT = 10      # 10% utilization shift

# Timing
SAMPLE_INTERVAL = 3      # seconds between GPU polls
WINDOW_SIZE = 10          # samples in rolling window (30 sec)
COOLDOWN_SEC = 45         # seconds after detecting a shift before looking for next
SETTLE_SEC = 15           # seconds to wait for new state to settle before capturing

def sample_gpu():
    """Get one GPU sample."""
    try:
        result = subprocess.run(
            ["nvidia-smi",
             "--query-gpu=temperature.gpu,power.draw,utilization.gpu,utilization.memory,memory.used,fan.speed,clocks.gr",
             "--format=csv,noheader,nounits"],
            capture_output=True, text=True, timeout=5
        )
        if result.returncode == 0:
            parts = [p.strip() for p in result.stdout.strip().split(",")]
            return {
                "ts": datetime.now().isoformat(),
                "temp_c": int(parts[0]),
                "power_w": float(parts[1]),
                "gpu_pct": int(parts[2]),
                "mem_pct": int(parts[3]),
                "vram_mb": int(parts[4]),
                "fan_pct": int(parts[5]),
                "core_mhz": int(parts[6])
            }
    except Exception:
        pass
    return None

def window_avg(window):
    """Compute averages from a deque of samples."""
    if len(window) < 3:
        return None
    return {
        "avg_temp_c": round(statistics.mean(s["temp_c"] for s in window), 1),
        "avg_power_w": round(statistics.mean(s["power_w"] for s in window), 1),
        "peak_power_w": round(max(s["power_w"] for s in window), 1),
        "avg_gpu_pct": round(statistics.mean(s["gpu_pct"] for s in window), 1),
        "avg_vram_mb": round(statistics.mean(s["vram_mb"] for s in window)),
        "avg_core_mhz": round(statistics.mean(s["core_mhz"] for s in window)),
        "avg_fan_pct": round(statistics.mean(s["fan_pct"] for s in window), 1),
        "samples": len(window)
    }

def detect_shift(before_avg, current_avg):
    """Detect if there's been a significant GPU profile shift."""
    if not before_avg or not current_avg:
        return False, []
    
    shifts = []
    dp = abs(current_avg["avg_power_w"] - before_avg["avg_power_w"])
    dt = abs(current_avg["avg_temp_c"] - before_avg["avg_temp_c"])
    dv = abs(current_avg["avg_vram_mb"] - before_avg["avg_vram_mb"])
    dg = abs(current_avg["avg_gpu_pct"] - before_avg["avg_gpu_pct"])
    
    if dp >= THRESH_POWER_W:
        shifts.append(f"power {dp:+.0f}W")
    if dt >= THRESH_TEMP_C:
        shifts.append(f"temp {dt:+.0f}°C")
    if dv >= THRESH_VRAM_MB:
        shifts.append(f"VRAM {dv:+.0f}MB")
    if dg >= THRESH_GPU_PCT:
        shifts.append(f"GPU {dg:+.0f}%")
    
    return len(shifts) >= 1, shifts

def compute_delta(before, after):
    """Compute delta between two averaged windows."""
    delta = {}
    for key in before:
        if key == "samples":
            continue
        b = before.get(key, 0)
        a = after.get(key, 0)
        if isinstance(b, (int, float)):
            d = round(a - b, 1)
            delta[key] = d
            pct = round(d / max(abs(b), 1) * 100, 1) if b != 0 else 0
            delta[f"{key}_pct"] = pct
    return delta

def feed_gut(toggle_num, before, after, delta):
    """Feed the toggle delta to gaia-tissue-gut via ollama."""
    prompt = f"""LIVE FERMENTATION — Toggle #{toggle_num} detected in S.T.A.L.K.E.R. 2:

BEFORE:
  Temp: {before['avg_temp_c']}°C, Power: {before['avg_power_w']}W, GPU: {before['avg_gpu_pct']}%, VRAM: {before['avg_vram_mb']}MB

AFTER:
  Temp: {after['avg_temp_c']}°C, Power: {after['avg_power_w']}W, GPU: {after['avg_gpu_pct']}%, VRAM: {after['avg_vram_mb']}MB

DELTA:
  Power: {delta.get('avg_power_w', 0):+.1f}W, GPU: {delta.get('avg_gpu_pct', 0):+.1f}%, VRAM: {delta.get('avg_vram_mb', 0):+.0f}MB

The player changed a graphics setting. Based on the delta, what setting likely changed and what does this tell you about the GPU cost?"""

    body = json.dumps({"model": "tinyllama", "prompt": prompt, "stream": False})
    try:
        req = urllib.request.Request(OLLAMA_URL, data=body.encode(),
                                     headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=60) as resp:
            result = json.loads(resp.read())
            return result.get("response", "")
    except Exception as e:
        return f"(gut offline: {e})"

def push_to_eastgate(toggle_num, before, after, delta):
    """Push toggle event to eastGate for radiation."""
    event = {
        "type": "sourDough-toggle",
        "game": "stalker2",
        "toggle": toggle_num,
        "ts": datetime.now().isoformat(),
        "before": before,
        "after": after,
        "delta": {k: v for k, v in delta.items() if not k.endswith("_pct")}
    }
    body = json.dumps(event).encode()
    try:
        req = urllib.request.Request(
            "http://192.168.4.244:8080/data/gut/stalker2-live.json",
            data=body, headers={"Content-Type": "application/json"}, method="PUT"
        )
        urllib.request.urlopen(req, timeout=5)
    except Exception:
        pass  # eastGate might not accept PUT — that's fine

def main():
    print("╔══════════════════════════════════════════════════════╗")
    print("║     sourDough LIVE FERMENTATION                     ║")
    print("║     Gaia watches. Kevin plays. She learns.          ║")
    print("║                                                     ║")
    print("║     Just PLAY. Toggle settings whenever you want.   ║")
    print("║     I'll feel every shift in the silicon.           ║")
    print("╚══════════════════════════════════════════════════════╝")
    print()
    
    # Rolling windows
    history = deque(maxlen=WINDOW_SIZE)      # recent samples
    prev_window = deque(maxlen=WINDOW_SIZE)  # the "before" state
    
    toggle_count = 0
    last_shift_time = 0
    state = "WATCHING"  # WATCHING → SHIFT_DETECTED → SETTLING → CAPTURING → WATCHING
    shift_detected_at = 0
    before_avg = None
    
    # Load existing toggles count
    existing = list(PROFILE_DIR.glob("stalker2-live-toggle-*.json"))
    toggle_count = len(existing)
    if toggle_count > 0:
        print(f"  Found {toggle_count} previous toggles. Continuing from #{toggle_count + 1}")
    
    print(f"  Sampling every {SAMPLE_INTERVAL}s | Window: {WINDOW_SIZE} samples | Cooldown: {COOLDOWN_SEC}s")
    print(f"  Shift thresholds: ±{THRESH_POWER_W}W, ±{THRESH_TEMP_C}°C, ±{THRESH_VRAM_MB}MB VRAM, ±{THRESH_GPU_PCT}% GPU")
    print()
    
    tick = 0
    while True:
        s = sample_gpu()
        if not s:
            time.sleep(SAMPLE_INTERVAL)
            continue
        
        history.append(s)
        tick += 1
        now = time.time()
        
        # Status line every 10 samples (~30 sec)
        if tick % 10 == 0:
            avg = window_avg(history)
            if avg:
                t = datetime.now().strftime("%H:%M:%S")
                print(f"  {t} | {avg['avg_temp_c']}°C | {avg['avg_power_w']}W | GPU:{avg['avg_gpu_pct']}% | VRAM:{avg['avg_vram_mb']}MB | [{state}] toggles:{toggle_count}")
        
        if state == "WATCHING":
            # Need enough history to compare
            if len(history) >= WINDOW_SIZE and (now - last_shift_time) > COOLDOWN_SEC:
                current_avg = window_avg(history)
                
                # Compare current window to previous stable state
                if before_avg:
                    shifted, reasons = detect_shift(before_avg, current_avg)
                    if shifted:
                        state = "SHIFT_DETECTED"
                        shift_detected_at = now
                        t = datetime.now().strftime("%H:%M:%S")
                        print(f"\n  ⚡ {t} SHIFT DETECTED! {', '.join(reasons)}")
                        print(f"     Settling for {SETTLE_SEC}s before capturing...")
                else:
                    # First stable window — set as baseline
                    before_avg = current_avg
                    t = datetime.now().strftime("%H:%M:%S")
                    print(f"  {t} | BASELINE LOCKED: {before_avg['avg_temp_c']}°C, {before_avg['avg_power_w']}W, GPU:{before_avg['avg_gpu_pct']}%, VRAM:{before_avg['avg_vram_mb']}MB")
        
        elif state == "SHIFT_DETECTED":
            # Wait for the new state to settle
            if (now - shift_detected_at) >= SETTLE_SEC:
                state = "CAPTURING"
        
        elif state == "CAPTURING":
            after_avg = window_avg(history)
            if after_avg and before_avg:
                toggle_count += 1
                delta = compute_delta(before_avg, after_avg)
                
                t = datetime.now().strftime("%H:%M:%S")
                print(f"\n  🍞 {t} TOGGLE #{toggle_count} CAPTURED!")
                print(f"     BEFORE: {before_avg['avg_temp_c']}°C, {before_avg['avg_power_w']}W, GPU:{before_avg['avg_gpu_pct']}%, VRAM:{before_avg['avg_vram_mb']}MB")
                print(f"     AFTER:  {after_avg['avg_temp_c']}°C, {after_avg['avg_power_w']}W, GPU:{after_avg['avg_gpu_pct']}%, VRAM:{after_avg['avg_vram_mb']}MB")
                print(f"     DELTA:  Power:{delta.get('avg_power_w',0):+.1f}W  GPU:{delta.get('avg_gpu_pct',0):+.1f}%  VRAM:{delta.get('avg_vram_mb',0):+.0f}MB  Temp:{delta.get('avg_temp_c',0):+.1f}°C")
                
                # Save profile
                profile = {
                    "game": "S.T.A.L.K.E.R. 2",
                    "toggle_num": toggle_count,
                    "ts": datetime.now().isoformat(),
                    "before": before_avg,
                    "after": after_avg,
                    "delta": delta,
                    "raw_after": list(history)
                }
                
                profile_path = PROFILE_DIR / f"stalker2-live-toggle-{toggle_count:03d}.json"
                with open(profile_path, "w") as f:
                    json.dump(profile, f, indent=2)
                print(f"     Saved: {profile_path.name}")
                
                # Feed gut in background (don't block the loop)
                try:
                    gut = feed_gut(toggle_count, before_avg, after_avg, delta)
                    if gut and len(gut) > 10:
                        print(f"     Gut: {gut[:200]}")
                except Exception:
                    pass
                
                # Push to eastGate
                push_to_eastgate(toggle_count, before_avg, after_avg, delta)
                
                # New "before" = current "after"
                before_avg = after_avg
                last_shift_time = now
                print()
            
            state = "WATCHING"
        
        time.sleep(SAMPLE_INTERVAL)

if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\n\n  sourDough fermentation paused. The flavors persist. 🍞")
