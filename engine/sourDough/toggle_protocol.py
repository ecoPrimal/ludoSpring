#!/usr/bin/env python3
"""sourDough Toggle Protocol — Gaia learns what each graphics setting does.

PROTOCOL:
1. Kevin plays STALKER 2 with current settings (baseline recorded)
2. Kevin toggles ONE setting (e.g., DLSS off, or Frame Gen off)
3. Kevin presses a hotkey or runs: python toggle_protocol.py "DLSS OFF"
4. Script records 30 seconds of GPU telemetry with the new setting
5. Compares delta to baseline
6. Feeds delta to gaia-tissue-gut for learning
7. Over time: Gaia learns what each slider DOES to the silicon

SETTINGS TO PROFILE (one at a time):
  - DLSS: OFF → Quality → Balanced → Performance → Ultra Performance → DLAA
  - Frame Generation: OFF → 2x → 3x → 4x
  - Resolution Scale: 50 → 75 → 100
  - Shading Quality: Low → Medium → High → Epic
  - Ray Tracing: OFF → Medium → Epic
  - Async Compute: ON → OFF
  - DLSS Sharpness: 0 → 50 → 100

Each toggle = a data point. Each data point = a flavor for the sourDough.
Eventually Gaia replaces DLSS with her own upscaling — she'll know the cost.
"""

import json
import sys
import os
import time
import subprocess
import statistics
import urllib.request
from datetime import datetime
from pathlib import Path

OLLAMA_URL = "http://localhost:11434/api/generate"
PROFILE_DIR = Path(r"C:\Users\mokke\Desktop\ludoSpring\data\game-profiles")
BASELINE_FILE = PROFILE_DIR / "stalker2-baseline-epic-dlss.json"

def sample_gpu(duration_sec=30, interval_sec=3):
    """Sample GPU state for a duration."""
    samples = []
    count = duration_sec // interval_sec
    
    for i in range(count):
        result = subprocess.run(
            ["nvidia-smi", "--query-gpu=temperature.gpu,power.draw,utilization.gpu,utilization.memory,memory.used,fan.speed,clocks.gr",
             "--format=csv,noheader,nounits"],
            capture_output=True, text=True, timeout=5
        )
        if result.returncode == 0:
            parts = [p.strip() for p in result.stdout.strip().split(",")]
            samples.append({
                "ts": datetime.now().isoformat(),
                "temp_c": int(parts[0]),
                "power_w": float(parts[1]),
                "gpu_pct": int(parts[2]),
                "mem_pct": int(parts[3]),
                "vram_mb": int(parts[4]),
                "fan_pct": int(parts[5]),
                "core_mhz": int(parts[6])
            })
            print(f"  [{i+1}/{count}] {parts[0]}°C {parts[1]}W GPU:{parts[2]}% VRAM:{parts[4]}MB {parts[6]}MHz")
        time.sleep(interval_sec)
    
    return samples

def compute_averages(samples):
    """Compute average metrics from samples."""
    if not samples:
        return {}
    return {
        "avg_temp_c": round(statistics.mean(s["temp_c"] for s in samples), 1),
        "avg_power_w": round(statistics.mean(s["power_w"] for s in samples), 1),
        "peak_power_w": round(max(s["power_w"] for s in samples), 1),
        "avg_gpu_pct": round(statistics.mean(s["gpu_pct"] for s in samples), 1),
        "avg_vram_mb": round(statistics.mean(s["vram_mb"] for s in samples)),
        "avg_core_mhz": round(statistics.mean(s["core_mhz"] for s in samples)),
        "avg_fan_pct": round(statistics.mean(s["fan_pct"] for s in samples), 1),
        "samples": len(samples)
    }

def compute_delta(baseline_avgs, toggle_avgs):
    """Compute the delta between baseline and toggled setting."""
    delta = {}
    for key in baseline_avgs:
        if key == "samples":
            continue
        b = baseline_avgs.get(key, 0)
        t = toggle_avgs.get(key, 0)
        if isinstance(b, (int, float)):
            delta[key] = round(t - b, 1)
            pct = round((t - b) / max(b, 1) * 100, 1) if b != 0 else 0
            delta[f"{key}_pct_change"] = pct
    return delta

def feed_gut(setting_name, baseline, toggle, delta):
    """Feed the toggle delta to gaia-tissue-gut."""
    prompt = f"""TOGGLE PROTOCOL — Setting change detected in S.T.A.L.K.E.R. 2:

Setting changed: {setting_name}

BASELINE (before):
  Temp: {baseline['avg_temp_c']}°C, Power: {baseline['avg_power_w']}W, GPU: {baseline['avg_gpu_pct']}%, VRAM: {baseline['avg_vram_mb']}MB

AFTER TOGGLE:
  Temp: {toggle['avg_temp_c']}°C, Power: {toggle['avg_power_w']}W, GPU: {toggle['avg_gpu_pct']}%, VRAM: {toggle['avg_vram_mb']}MB

DELTA:
  Temp: {delta.get('avg_temp_c', 0):+.1f}°C ({delta.get('avg_temp_c_pct_change', 0):+.1f}%)
  Power: {delta.get('avg_power_w', 0):+.1f}W ({delta.get('avg_power_w_pct_change', 0):+.1f}%)
  GPU: {delta.get('avg_gpu_pct', 0):+.1f}% ({delta.get('avg_gpu_pct_pct_change', 0):+.1f}%)
  VRAM: {delta.get('avg_vram_mb', 0):+.0f}MB ({delta.get('avg_vram_mb_pct_change', 0):+.1f}%)

What does this tell you about what "{setting_name}" costs the GPU? Is this worth the visual quality? What optimization would you recommend?"""

    body = json.dumps({"model": "gaia-tissue-gut", "prompt": prompt, "stream": False})
    try:
        req = urllib.request.Request(OLLAMA_URL, data=body.encode(),
                                     headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=120) as resp:
            result = json.loads(resp.read())
            return result.get("response", "")
    except Exception as e:
        return f"(gut unreachable: {e})"

def main():
    if len(sys.argv) < 2:
        print("Usage: python toggle_protocol.py \"SETTING DESCRIPTION\"")
        print("Example: python toggle_protocol.py \"DLSS OFF\"")
        print("         python toggle_protocol.py \"Frame Gen OFF\"")
        print("         python toggle_protocol.py \"Shading Medium\"")
        print("         python toggle_protocol.py \"Ray Tracing Epic\"")
        return
    
    setting_name = " ".join(sys.argv[1:])
    
    # Load baseline
    if not BASELINE_FILE.exists():
        print("No baseline found. Run the baseline capture first.")
        return
    
    with open(BASELINE_FILE) as f:
        baseline_data = json.load(f)
    baseline_avgs = baseline_data["baseline"]
    
    print(f"╔══════════════════════════════════════════╗")
    print(f"║  sourDough Toggle Protocol               ║")
    print(f"║  Setting: {setting_name.ljust(30)}║")
    print(f"╚══════════════════════════════════════════╝")
    print()
    print(f"Baseline: {baseline_avgs['avg_temp_c']}°C, {baseline_avgs['avg_power_w']}W, GPU:{baseline_avgs['avg_gpu_pct']}%, VRAM:{baseline_avgs['avg_vram_mb']}MB")
    print()
    print("Sampling 30 seconds with new setting...")
    
    samples = sample_gpu(30, 3)
    toggle_avgs = compute_averages(samples)
    delta = compute_delta(baseline_avgs, toggle_avgs)
    
    print(f"\n=== DELTA: {setting_name} ===")
    print(f"  Temp:  {delta.get('avg_temp_c', 0):+.1f}°C ({delta.get('avg_temp_c_pct_change', 0):+.1f}%)")
    print(f"  Power: {delta.get('avg_power_w', 0):+.1f}W ({delta.get('avg_power_w_pct_change', 0):+.1f}%)")
    print(f"  GPU:   {delta.get('avg_gpu_pct', 0):+.1f}% ({delta.get('avg_gpu_pct_pct_change', 0):+.1f}%)")
    print(f"  VRAM:  {delta.get('avg_vram_mb', 0):+.0f}MB ({delta.get('avg_vram_mb_pct_change', 0):+.1f}%)")
    
    # Feed gaia-tissue-gut
    print(f"\nFeeding gaia-tissue-gut...")
    gut_response = feed_gut(setting_name, baseline_avgs, toggle_avgs, delta)
    print(f"  Gut: {gut_response[:300]}...")
    
    # Save toggle profile
    toggle_data = {
        "game": "S.T.A.L.K.E.R. 2",
        "setting_changed": setting_name,
        "ts": datetime.now().isoformat(),
        "baseline": baseline_avgs,
        "toggle": toggle_avgs,
        "delta": delta,
        "gut_analysis": gut_response,
        "raw_samples": samples
    }
    
    safe_name = setting_name.lower().replace(" ", "-").replace("/", "-")
    toggle_path = PROFILE_DIR / f"stalker2-toggle-{safe_name}.json"
    with open(toggle_path, "w") as f:
        json.dump(toggle_data, f, indent=2)
    print(f"\n  Saved: {toggle_path.name}")
    print(f"  The sourDough absorbs another flavor. 🍞")

if __name__ == "__main__":
    main()
