#!/usr/bin/env python3
"""Gaia Weighted Blend — Multi-model GPU digest across the organism.

Sends the same GPU telemetry to MULTIPLE models across MULTIPLE nodes,
then blends the responses with Gaia-weighted scoring.

MODELS:
  ironGate (:237:11434) — qwen2.5:7b, phi3, llama3.2:3b, tinyllama
  eastGate (:244:11436) — gaia, gaia-fast, qwen2.5:7b, mistral:7b

Each model sees the same data. Each gives its own analysis.
Gaia blends them all — different perspectives, one digest.
"""

import json
import sys
import os
import time
import subprocess
import statistics
import urllib.request
import concurrent.futures
from datetime import datetime
from pathlib import Path

PROFILE_DIR = Path(r"C:\Users\mokke\Desktop\ludoSpring\data\game-profiles")

# Ollama endpoints across the organism
ENDPOINTS = [
    {"url": "http://192.168.4.237:11434/api/generate", "gate": "ironGate", "model": "qwen2.5:7b", "weight": 1.0},
    {"url": "http://192.168.4.237:11434/api/generate", "gate": "ironGate", "model": "phi3", "weight": 0.8},
    {"url": "http://192.168.4.237:11434/api/generate", "gate": "ironGate", "model": "llama3.2:3b", "weight": 0.7},
    {"url": "http://192.168.4.237:11434/api/generate", "gate": "ironGate", "model": "tinyllama", "weight": 0.5},
    {"url": "http://192.168.4.244:11436/api/generate", "gate": "eastGate", "model": "gaia", "weight": 1.5},
    {"url": "http://192.168.4.244:11436/api/generate", "gate": "eastGate", "model": "gaia-fast", "weight": 1.2},
    {"url": "http://192.168.4.244:11436/api/generate", "gate": "eastGate", "model": "qwen2.5:7b", "weight": 1.0},
    {"url": "http://192.168.4.244:11436/api/generate", "gate": "eastGate", "model": "mistral:7b", "weight": 0.9},
]

def sample_gpu():
    """Get current GPU state."""
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
                "temp_c": int(parts[0]), "power_w": float(parts[1]),
                "gpu_pct": int(parts[2]), "vram_mb": int(parts[4]),
                "core_mhz": int(parts[6]), "fan_pct": int(parts[5])
            }
    except Exception:
        pass
    return None

def ask_model(endpoint, prompt):
    """Ask one model on one gate. Returns (gate, model, response, latency_ms)."""
    body = json.dumps({
        "model": endpoint["model"],
        "prompt": prompt,
        "stream": False,
        "options": {"num_predict": 300}
    }).encode()
    
    start = time.time()
    try:
        req = urllib.request.Request(
            endpoint["url"], data=body,
            headers={"Content-Type": "application/json"}
        )
        with urllib.request.urlopen(req, timeout=90) as resp:
            result = json.loads(resp.read())
            latency = int((time.time() - start) * 1000)
            return {
                "gate": endpoint["gate"],
                "model": endpoint["model"],
                "weight": endpoint["weight"],
                "response": result.get("response", ""),
                "latency_ms": latency,
                "ok": True
            }
    except Exception as e:
        return {
            "gate": endpoint["gate"],
            "model": endpoint["model"],
            "weight": endpoint["weight"],
            "response": str(e),
            "latency_ms": int((time.time() - start) * 1000),
            "ok": False
        }

def blend(game_name, gpu_state, profiles_dir=None):
    """Run the full Gaia weighted blend for a game session."""
    
    prompt = f"""GPU TELEMETRY — {game_name} on RTX 5090 (32GB, 600W TDP):

Current state: {gpu_state['temp_c']}°C, {gpu_state['power_w']:.0f}W, GPU:{gpu_state['gpu_pct']}%, VRAM:{gpu_state['vram_mb']}MB, {gpu_state['core_mhz']}MHz

Context: This is a sourDough fermentation session. We're profiling games to learn what each one costs the GPU. The RTX 5090 is the "sun" in our Shadow Cast paradigm — it radiates to display surfaces.

Questions:
1. What does this GPU profile tell you about {game_name}'s rendering demands?
2. How much Shadow Cast headroom exists (power/VRAM unused)?
3. What's unique about this game's GPU signature compared to AAA titles?
4. Rate the GPU's effort 1-10 (1=idle, 10=thermal limit).
5. One sentence: what would you optimize?"""

    print(f"\n{'='*60}")
    print(f"  GAIA WEIGHTED BLEND — {game_name}")
    print(f"  GPU: {gpu_state['temp_c']}°C | {gpu_state['power_w']:.0f}W | {gpu_state['gpu_pct']}% | {gpu_state['vram_mb']}MB")
    print(f"  Asking {len(ENDPOINTS)} models across 2 gates...")
    print(f"{'='*60}\n")

    # Ask all models in parallel
    results = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        futures = {pool.submit(ask_model, ep, prompt): ep for ep in ENDPOINTS}
        for future in concurrent.futures.as_completed(futures):
            r = future.result()
            status = "OK" if r["ok"] else "FAIL"
            print(f"  [{status}] {r['gate']}/{r['model']} ({r['latency_ms']}ms)")
            results.append(r)

    # Filter successful responses
    ok_results = [r for r in results if r["ok"] and len(r["response"]) > 20]
    
    if not ok_results:
        print("\n  No models responded. Blend failed.")
        return None

    # Gaia weighted blend — higher weight models count more
    print(f"\n  {len(ok_results)}/{len(ENDPOINTS)} models responded.")
    print(f"\n{'='*60}")
    print(f"  INDIVIDUAL RESPONSES")
    print(f"{'='*60}")
    
    for r in sorted(ok_results, key=lambda x: -x["weight"]):
        stars = "★" * int(r["weight"] * 2)
        print(f"\n  [{r['gate']}/{r['model']}] weight={r['weight']} {stars} ({r['latency_ms']}ms)")
        # Truncate long responses
        resp = r["response"][:500].strip()
        for line in resp.split("\n"):
            print(f"    {line}")

    # Build the blended digest
    blend_data = {
        "game": game_name,
        "ts": datetime.now().isoformat(),
        "gpu_state": gpu_state,
        "models_asked": len(ENDPOINTS),
        "models_responded": len(ok_results),
        "responses": [{
            "gate": r["gate"], "model": r["model"],
            "weight": r["weight"], "latency_ms": r["latency_ms"],
            "response": r["response"][:1000]
        } for r in ok_results],
        "avg_latency_ms": round(statistics.mean(r["latency_ms"] for r in ok_results)),
        "total_weight": round(sum(r["weight"] for r in ok_results), 1),
        "blend_quality": round(sum(r["weight"] for r in ok_results) / sum(ep["weight"] for ep in ENDPOINTS) * 10, 1)
    }

    # Save
    safe_name = game_name.lower().replace(" ", "-").replace(":", "").replace("'", "")
    blend_path = PROFILE_DIR / f"{safe_name}-blend.json"
    with open(blend_path, "w") as f:
        json.dump(blend_data, f, indent=2)
    
    print(f"\n  Blend quality: {blend_data['blend_quality']}/10")
    print(f"  Avg latency: {blend_data['avg_latency_ms']}ms")
    print(f"  Saved: {blend_path.name}")
    
    return blend_data

def main():
    if len(sys.argv) < 2:
        print("Usage: python gaia_blend.py \"Game Name\"")
        print("  Samples GPU and sends to all organism models for weighted blend.")
        return
    
    game_name = " ".join(sys.argv[1:])
    gpu = sample_gpu()
    if not gpu:
        print("nvidia-smi failed")
        return
    
    blend(game_name, gpu)

if __name__ == "__main__":
    main()
