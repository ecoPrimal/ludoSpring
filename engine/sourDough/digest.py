#!/usr/bin/env python3
"""sourDough Telemetry Digester — ferments GPU telemetry into organism knowledge.

Reads GPU watcher JSONL sessions, analyzes patterns, feeds gaia-tissue-gut.
The old session feeds the new. Mash never boots dead.
"""

import json
import glob
import os
import sys
import statistics
from datetime import datetime
from pathlib import Path
import urllib.request

TELEMETRY_DIR = Path(os.environ.get("TELEMETRY_DIR", 
    r"C:\Users\mokke\Desktop\Gaia\data\gpu-telemetry"))
OLLAMA_URL = os.environ.get("OLLAMA_URL", "http://localhost:11434/api/generate")
OUTPUT_DIR = Path(os.environ.get("OUTPUT_DIR",
    r"C:\Users\mokke\Desktop\ludoSpring\data\digested"))

def load_session(path):
    """Load a JSONL telemetry session."""
    samples = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if line:
                try:
                    samples.append(json.loads(line))
                except json.JSONDecodeError:
                    continue
    return samples

def analyze_session(samples):
    """Analyze a telemetry session — extract thermal curves, power patterns, VRAM usage."""
    if not samples:
        return None
    
    temps = [s["temp_c"] for s in samples]
    powers = [s["power_w"] for s in samples]
    gpu_utils = [s["gpu_util_pct"] for s in samples]
    vrams = [s["vram_used_mb"] for s in samples]
    clocks = [s.get("clock_core_mhz", 0) for s in samples]
    
    duration_min = len(samples) * 5 / 60  # 5 sec intervals
    
    # Detect phases: idle, loading, shader compile, gameplay, cutscene
    phases = []
    for i, s in enumerate(samples):
        if s["gpu_util_pct"] < 5 and s["power_w"] < 80:
            phases.append("idle")
        elif s["gpu_util_pct"] > 80:
            phases.append("heavy_render")
        elif 30 < s["gpu_util_pct"] < 80 and s["power_w"] > 150:
            phases.append("shader_compile")
        elif s["vram_used_mb"] > 10000:
            phases.append("texture_streaming")
        else:
            phases.append("light_load")
    
    # Thermal curve analysis
    thermal_ramp = None
    if len(temps) > 20:
        start_temp = statistics.mean(temps[:5])
        peak_temp = max(temps)
        ramp_idx = temps.index(peak_temp)
        if ramp_idx > 0:
            thermal_ramp = {
                "start_c": round(start_temp, 1),
                "peak_c": peak_temp,
                "ramp_time_sec": ramp_idx * 5,
                "rate_c_per_min": round((peak_temp - start_temp) / (ramp_idx * 5 / 60), 2) if ramp_idx > 0 else 0
            }
    
    # Power efficiency
    power_phases = {}
    for phase in set(phases):
        idxs = [i for i, p in enumerate(phases) if p == phase]
        if idxs:
            phase_powers = [powers[i] for i in idxs]
            phase_gpus = [gpu_utils[i] for i in idxs]
            power_phases[phase] = {
                "avg_power_w": round(statistics.mean(phase_powers), 1),
                "avg_gpu_pct": round(statistics.mean(phase_gpus), 1),
                "samples": len(idxs),
                "efficiency": round(statistics.mean(phase_gpus) / max(statistics.mean(phase_powers), 1), 3)
            }
    
    return {
        "session_file": "",
        "samples": len(samples),
        "duration_min": round(duration_min, 1),
        "ts_start": samples[0].get("ts", ""),
        "ts_end": samples[-1].get("ts", ""),
        "thermal": {
            "min_c": min(temps),
            "max_c": max(temps),
            "avg_c": round(statistics.mean(temps), 1),
            "ramp": thermal_ramp
        },
        "power": {
            "min_w": round(min(powers), 1),
            "max_w": round(max(powers), 1),
            "avg_w": round(statistics.mean(powers), 1),
            "total_wh": round(sum(powers) * 5 / 3600, 2)
        },
        "gpu_utilization": {
            "min_pct": min(gpu_utils),
            "max_pct": max(gpu_utils),
            "avg_pct": round(statistics.mean(gpu_utils), 1)
        },
        "vram": {
            "min_mb": min(vrams),
            "max_mb": max(vrams),
            "avg_mb": round(statistics.mean(vrams)),
            "peak_gb": round(max(vrams) / 1024, 2)
        },
        "phases": power_phases,
        "phase_timeline": phases[:20]  # first 20 for preview
    }

def feed_gut(analysis):
    """Feed analysis to gaia-tissue-gut via ollama."""
    prompt = f"""GPU SESSION DIGEST — sourDough fermentation output:

Duration: {analysis['duration_min']} min, {analysis['samples']} samples
Thermal: {analysis['thermal']['min_c']}→{analysis['thermal']['max_c']}°C (avg {analysis['thermal']['avg_c']}°C)
Power: {analysis['power']['min_w']}→{analysis['power']['max_w']}W (avg {analysis['power']['avg_w']}W, total {analysis['power']['total_wh']}Wh)
GPU: {analysis['gpu_utilization']['avg_pct']}% avg, {analysis['gpu_utilization']['max_pct']}% peak
VRAM: {analysis['vram']['peak_gb']}GB peak

Phases detected: {', '.join(f"{k}: {v['samples']} samples, {v['avg_power_w']}W avg" for k,v in analysis['phases'].items())}

Analyze: What game behavior patterns do you see? What's the thermal-to-performance relationship? Where are optimization opportunities? This is STALKER 2 on RTX 5090."""

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
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    
    sessions = sorted(glob.glob(str(TELEMETRY_DIR / "session-*.jsonl")))
    if not sessions:
        print("No telemetry sessions found.")
        return
    
    print(f"sourDough: Found {len(sessions)} session(s)")
    
    for session_path in sessions:
        session_name = Path(session_path).stem
        digest_path = OUTPUT_DIR / f"{session_name}-digest.json"
        
        if digest_path.exists():
            print(f"  {session_name}: already digested")
            continue
        
        print(f"  Fermenting: {session_name}...")
        samples = load_session(session_path)
        if len(samples) < 10:
            print(f"    Too few samples ({len(samples)}), skipping")
            continue
        
        analysis = analyze_session(samples)
        analysis["session_file"] = session_name
        
        # Feed the gut
        print(f"    Feeding gaia-tissue-gut...")
        gut_response = feed_gut(analysis)
        analysis["gut_analysis"] = gut_response
        print(f"    Gut says: {gut_response[:200]}...")
        
        # Save digest
        with open(digest_path, "w") as f:
            json.dump(analysis, f, indent=2)
        print(f"    Digested → {digest_path.name}")
    
    print("\nsourDough fermentation complete. The old feeds the new. 🍞")

if __name__ == "__main__":
    main()
