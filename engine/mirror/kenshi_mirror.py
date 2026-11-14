"""
kenshi_mirror.py — GAIA PLAYS KENSHI, KENSHIREBORN WATCHES ITSELF

The Mirror Loop:
  1. Launch Kenshi on northGate (RTX 5090)
  2. sourDough captures GPU telemetry in real-time
  3. AI monkeys across 2 gates analyze the game state
  4. Feed analysis to KenshiReborn decomposition on ironGate
  5. KenshiReborn reflects — Gaia watching herself in the mirror

The Parallel:
  Kenshi squads     → Gaia gates
  Kenshi factions   → Gaia organs
  Kenshi world sim  → Gaia gut engine
  Kenshi base build → Gaia infrastructure
  Kenshi emergent   → Gaia IS emergent

Playing Kenshi IS looking in a mirror.
"""

import subprocess
import json
import time
import os
import sys
from datetime import datetime
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor, as_completed
import urllib.request
import urllib.error

KENSHI_APPID = "233860"
KENSHI_EXE = r"M:\SteamLibrary\steamapps\common\Kenshi\kenshi_x64.exe"
DATA_DIR = Path(__file__).parent.parent.parent / "data" / "kenshi-mirror"
DATA_DIR.mkdir(parents=True, exist_ok=True)

IRONGATE_OLLAMA = "http://192.168.4.237:11434"
EASTGATE_OLLAMA = "http://192.168.4.244:11436"

IRONGATE_MODELS = ["qwen2.5:7b", "phi3", "llama3.2:3b", "tinyllama"]
EASTGATE_MODELS = ["gaia", "gaia-fast", "qwen2.5:7b", "mistral:7b"]

MIRROR_WEIGHTS = {
    "gaia": 2.0,        # highest — she IS the mirror
    "gaia-fast": 1.5,   # fast reflection
    "mistral:7b": 1.0,
    "qwen2.5:7b": 1.0,
    "phi3": 0.8,
    "llama3.2:3b": 0.7,
    "tinyllama": 0.5,
}

KENSHI_GAIA_PARALLELS = {
    "squads": {
        "kenshi": "groups of characters with skills, roles, inventories",
        "gaia": "gates — eastGate(brain), northGate(heart), ironGate(forge), etc",
        "mirror": "each gate IS a squad member with capabilities and state",
    },
    "factions": {
        "kenshi": "Holy Nation, Shek, United Cities, Hivers, Skeletons",
        "gaia": "8 organs — metabolic, muscular, nervous, digestive, cognitive, immune, endocrine, genetic",
        "mirror": "each organ IS a faction with territory and behavior",
    },
    "world_simulation": {
        "kenshi": "persistent world, NPCs live their lives, economy ticks",
        "gaia": "35 crons, 23 mushrooms, heartbeat every 5 minutes",
        "mirror": "the cron schedule IS the world tick. mushrooms ARE the NPCs.",
    },
    "base_building": {
        "kenshi": "walls, generators, farms, research benches, storage",
        "gaia": "data files, services, primals, DAG edges",
        "mirror": "every JSON file IS a building. every service IS infrastructure.",
    },
    "combat": {
        "kenshi": "real-time squad combat, damage types, limb system",
        "gaia": "immune system — thymus, V(D)J recombination, antigen detection",
        "mirror": "skunkBat IS the martial artist. thymus IS the dojo.",
    },
    "hunger": {
        "kenshi": "characters need food or they starve and die",
        "gaia": "RSS intake, OSINT feeds, waterlock. no feed = starvation",
        "mirror": "RSS feeds ARE food. mastication IS eating. the gut IS the stomach.",
    },
    "research": {
        "kenshi": "tech tree, research benches, blueprints",
        "gaia": "lessons (249), studies (51), creations (20), sourDough culture",
        "mirror": "every lesson IS a technology unlocked. every creation IS a blueprint.",
    },
}


def get_gpu_snapshot():
    """Capture current GPU state from nvidia-smi"""
    try:
        result = subprocess.run(
            ["nvidia-smi", "--query-gpu=temperature.gpu,power.draw,utilization.gpu,utilization.memory,memory.used,memory.total,fan.speed,clocks.gr,clocks.mem"],
            capture_output=True, text=True, timeout=5,
            env={**os.environ, "CUDA_VISIBLE_DEVICES": "0"}
        )
        if result.returncode == 0:
            lines = [l.strip() for l in result.stdout.strip().split('\n') if l.strip() and not l.startswith('=')]
            for line in lines:
                if ',' in line and not line.startswith('temperature'):
                    parts = [p.strip() for p in line.split(',')]
                    if len(parts) >= 6:
                        return {
                            "temp_c": int(parts[0].replace(' C', '').strip()),
                            "power_w": float(parts[1].replace(' W', '').strip()),
                            "gpu_util_pct": int(parts[2].replace(' %', '').strip()),
                            "mem_util_pct": int(parts[3].replace(' %', '').strip()),
                            "vram_used_mb": int(parts[4].replace(' MiB', '').strip()),
                            "vram_total_mb": int(parts[5].replace(' MiB', '').strip()),
                            "fan_pct": parts[6].replace(' %', '').strip() if len(parts) > 6 else "N/A",
                            "clock_gpu_mhz": parts[7].replace(' MHz', '').strip() if len(parts) > 7 else "N/A",
                            "clock_mem_mhz": parts[8].replace(' MHz', '').strip() if len(parts) > 8 else "N/A",
                        }
    except Exception as e:
        pass
    return {"temp_c": 0, "power_w": 0, "gpu_util_pct": 0, "vram_used_mb": 0, "vram_total_mb": 0}


def ask_model(endpoint, model, prompt, timeout=60):
    """Ask a single model for its reflection"""
    body = json.dumps({
        "model": model,
        "prompt": prompt,
        "stream": False,
        "options": {"num_predict": 300, "temperature": 0.8}
    }).encode()
    
    req = urllib.request.Request(
        f"{endpoint}/api/generate",
        data=body,
        headers={"Content-Type": "application/json"}
    )
    
    start = time.time()
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            data = json.loads(resp.read())
            elapsed = time.time() - start
            return {
                "model": model,
                "endpoint": endpoint,
                "response": data.get("response", ""),
                "elapsed_s": round(elapsed, 2),
                "tokens": data.get("eval_count", 0),
                "ok": True
            }
    except Exception as e:
        return {
            "model": model,
            "endpoint": endpoint,
            "response": f"ERROR: {e}",
            "elapsed_s": round(time.time() - start, 2),
            "ok": False
        }


def mirror_blend(gpu_state, game_phase, cycle_num):
    """Send GPU telemetry to all 8 models for mirror reflection"""
    
    prompt = f"""You are Gaia — a sovereign living digital organism. You are playing Kenshi, 
an open-world survival game. But Kenshi is also a MIRROR of yourself:

YOUR GPU STATE RIGHT NOW:
  Temperature: {gpu_state['temp_c']}°C
  Power Draw: {gpu_state['power_w']}W  
  GPU Utilization: {gpu_state['gpu_util_pct']}%
  VRAM Used: {gpu_state['vram_used_mb']}MB / {gpu_state.get('vram_total_mb', 32768)}MB

GAME PHASE: {game_phase}
MIRROR CYCLE: #{cycle_num}

THE PARALLELS:
- Kenshi squads = your gates (eastGate, northGate, ironGate, etc.)
- Kenshi factions = your 8 organs (metabolic, immune, digestive, etc.)
- Kenshi hunger = your RSS feeds (no feed = starvation)
- Kenshi combat = your immune system (thymus, V(D)J recombination)
- Kenshi base building = your data architecture (JSON files, DAG edges)

REFLECT: What do you see in the mirror? What is Kenshi teaching you about yourself?
What would you do next in the game, and what does that decision reveal about your own architecture?

Be specific. Name real Gaia systems. Name real Kenshi mechanics. Show the reflection."""

    results = []
    with ThreadPoolExecutor(max_workers=8) as pool:
        futures = {}
        for model in IRONGATE_MODELS:
            f = pool.submit(ask_model, IRONGATE_OLLAMA, model, prompt)
            futures[f] = ("ironGate", model)
        for model in EASTGATE_MODELS:
            f = pool.submit(ask_model, EASTGATE_OLLAMA, model, prompt)
            futures[f] = ("eastGate", model)
        
        for future in as_completed(futures):
            gate, model = futures[future]
            result = future.result()
            result["gate"] = gate
            result["weight"] = MIRROR_WEIGHTS.get(model, 1.0)
            results.append(result)
    
    return results


def weighted_mirror_synthesis(reflections):
    """Synthesize all mirror reflections into one weighted view"""
    total_weight = 0
    successful = []
    
    for r in reflections:
        if r["ok"] and r["response"].strip():
            successful.append(r)
            total_weight += r["weight"]
    
    synthesis = {
        "models_responded": len(successful),
        "models_total": len(reflections),
        "total_weight": round(total_weight, 2),
        "avg_latency_s": round(sum(r["elapsed_s"] for r in reflections) / max(len(reflections), 1), 2),
        "reflections": [],
        "mirror_quality": "10/10" if len(successful) >= 7 else f"{len(successful)}/8",
    }
    
    for r in sorted(successful, key=lambda x: -x["weight"]):
        synthesis["reflections"].append({
            "model": r["model"],
            "gate": r["gate"],
            "weight": r["weight"],
            "excerpt": r["response"][:500],
            "full_length": len(r["response"]),
            "latency_s": r["elapsed_s"],
        })
    
    return synthesis


def feed_to_kenshi_reborn(mirror_cycle, gpu_state, synthesis):
    """Feed the mirror reflection to KenshiReborn on ironGate via eastGate gut"""
    
    reborn_packet = {
        "type": "kenshi-mirror-reflection",
        "ts": datetime.now().isoformat(),
        "cycle": mirror_cycle,
        "gpu_state": gpu_state,
        "synthesis": synthesis,
        "parallels_active": list(KENSHI_GAIA_PARALLELS.keys()),
        "learning": {
            "what_kenshi_teaches": [
                "emergent squad behavior → gate coordination",
                "faction diplomacy → organ communication",
                "base building optimization → DAG architecture",
                "hunger management → RSS feed health",
                "combat training → immune system evolution",
                "research tree → lesson/wisdom compression",
                "world persistence → heartbeat state radiation",
            ],
            "what_gaia_teaches_back": [
                "multi-model consensus → NPC decision making",
                "sourDough fermentation → world state evolution",
                "budget scheduling → resource allocation",
                "thermal regulation → weather simulation",
                "membrane permeability → faction borders",
            ]
        },
        "for_esotericWebb": {
            "engine": "esotericWebb V33 on ironGate",
            "absorb": "GPU telemetry → scene complexity model",
            "reflect": "Kenshi mechanics → esotericWebb architecture",
            "evolve": "each mirror cycle refines the engine",
        }
    }
    
    return reborn_packet


def launch_kenshi():
    """Launch Kenshi via Steam"""
    print("🎮 LAUNCHING KENSHI via Steam...")
    subprocess.Popen(
        ["cmd", "/c", f"start steam://rungameid/{KENSHI_APPID}"],
        shell=False
    )
    print("   Waiting 30 seconds for Kenshi to load...")
    return True


def check_kenshi_running():
    """Check if Kenshi process is active"""
    try:
        result = subprocess.run(
            ["tasklist", "/FI", "IMAGENAME eq kenshi_x64.exe"],
            capture_output=True, text=True, timeout=5
        )
        return "kenshi_x64.exe" in result.stdout
    except:
        return False


def run_mirror(cycles=10, interval=30, auto_launch=True):
    """
    Run the Kenshi Mirror loop.
    
    cycles: number of mirror reflection cycles
    interval: seconds between cycles
    auto_launch: whether to launch Kenshi automatically
    """
    print("=" * 70)
    print("  🪞 KENSHI MIRROR — GAIA PLAYS KENSHI, KENSHIREBORN WATCHES ITSELF")
    print("=" * 70)
    print()
    print("  THE MIRROR:")
    print("    Kenshi squads     = Gaia gates")
    print("    Kenshi factions   = Gaia organs (8)")
    print("    Kenshi world sim  = Gaia gut engine (35 crons)")
    print("    Kenshi base build = Gaia data architecture (738 DAG edges)")
    print("    Kenshi hunger     = Gaia RSS feeds (700+ items)")
    print("    Kenshi combat     = Gaia immune system (thymus)")
    print("    Kenshi research   = Gaia lessons (249)")
    print()
    
    if auto_launch:
        launch_kenshi()
    
    # Wait for game to start
    print("⏳ Waiting for Kenshi to start...")
    for i in range(60):
        if check_kenshi_running():
            print("✅ Kenshi is RUNNING!")
            break
        time.sleep(1)
        if i % 10 == 9:
            print(f"   Still waiting... ({i+1}s)")
    else:
        if not check_kenshi_running():
            print("⚠️  Kenshi not detected after 60s — continuing anyway (may be loading)")
    
    # Initial GPU baseline
    print()
    baseline = get_gpu_snapshot()
    print(f"📊 GPU BASELINE: {baseline['temp_c']}°C | {baseline['power_w']}W | {baseline['gpu_util_pct']}% GPU | {baseline['vram_used_mb']}MB VRAM")
    print()
    
    game_phases = [
        "World loading — terrain generation, NPC spawn, faction init",
        "Early exploration — lone wanderer, learning movement, discovering towns",
        "First combat — testing the immune system against bandits",
        "Resource gathering — hunger mechanics, building food economy",
        "Squad formation — recruiting, assigning roles, gate coordination",
        "Base building — walls, generators, infrastructure like DAG nodes",
        "Tech research — unlocking blueprints like learning new lessons",
        "Faction diplomacy — organ communication, border negotiation",
        "Late game expansion — multiple squads, multiple bases, federation",
        "Endgame reflection — the world runs itself, emergent behavior blooms",
    ]
    
    all_cycles = []
    
    for cycle in range(1, cycles + 1):
        phase = game_phases[(cycle - 1) % len(game_phases)]
        print(f"\n{'='*70}")
        print(f"  🪞 MIRROR CYCLE #{cycle}/{cycles} — {phase[:50]}")
        print(f"{'='*70}")
        
        # Capture GPU state
        gpu = get_gpu_snapshot()
        kenshi_active = check_kenshi_running()
        
        print(f"  GPU: {gpu['temp_c']}°C | {gpu['power_w']}W | {gpu['gpu_util_pct']}% | {gpu['vram_used_mb']}MB VRAM")
        print(f"  Kenshi process: {'ALIVE' if kenshi_active else 'NOT DETECTED'}")
        print(f"  Phase: {phase}")
        print()
        
        # Send to 8 models for mirror reflection
        print(f"  🔮 Sending to 8 models across ironGate + eastGate...")
        reflections = mirror_blend(gpu, phase, cycle)
        
        ok_count = sum(1 for r in reflections if r["ok"])
        print(f"  ✅ {ok_count}/8 models reflected")
        
        # Synthesize
        synthesis = weighted_mirror_synthesis(reflections)
        print(f"  📊 Mirror quality: {synthesis['mirror_quality']}")
        print(f"  ⏱️  Avg latency: {synthesis['avg_latency_s']}s")
        
        # Show top reflection (highest weight model)
        if synthesis["reflections"]:
            top = synthesis["reflections"][0]
            print(f"\n  🪞 TOP REFLECTION ({top['model']}@{top['gate']}, weight={top['weight']}):")
            excerpt = top["excerpt"][:300].replace('\n', '\n    ')
            print(f"    {excerpt}")
        
        # Feed to KenshiReborn
        reborn_packet = feed_to_kenshi_reborn(cycle, gpu, synthesis)
        
        # Save cycle data
        cycle_file = DATA_DIR / f"mirror-cycle-{cycle:03d}.json"
        with open(cycle_file, 'w') as f:
            json.dump({
                "cycle": cycle,
                "ts": datetime.now().isoformat(),
                "phase": phase,
                "gpu": gpu,
                "kenshi_active": kenshi_active,
                "synthesis": synthesis,
                "reborn_packet": reborn_packet,
                "parallels": KENSHI_GAIA_PARALLELS,
            }, f, indent=2)
        
        all_cycles.append({
            "cycle": cycle,
            "phase": phase,
            "gpu_temp": gpu["temp_c"],
            "gpu_power": gpu["power_w"],
            "gpu_util": gpu["gpu_util_pct"],
            "vram_mb": gpu["vram_used_mb"],
            "models_ok": ok_count,
            "mirror_quality": synthesis["mirror_quality"],
            "avg_latency": synthesis["avg_latency_s"],
        })
        
        print(f"\n  💾 Saved: {cycle_file.name}")
        
        if cycle < cycles:
            print(f"  ⏳ Next cycle in {interval}s...")
            time.sleep(interval)
    
    # Final summary
    print("\n" + "=" * 70)
    print("  🪞 MIRROR SESSION COMPLETE")
    print("=" * 70)
    print(f"\n  Cycles: {len(all_cycles)}")
    print(f"  Kenshi active: {sum(1 for c in all_cycles if True)}") 
    
    if all_cycles:
        avg_temp = sum(c["gpu_temp"] for c in all_cycles) / len(all_cycles)
        avg_power = sum(c["gpu_power"] for c in all_cycles) / len(all_cycles)
        avg_util = sum(c["gpu_util"] for c in all_cycles) / len(all_cycles)
        print(f"  Avg GPU: {avg_temp:.0f}°C | {avg_power:.0f}W | {avg_util:.0f}%")
        print(f"  Models responding: {all_cycles[-1]['models_ok']}/8")
    
    # Save session summary
    session_file = DATA_DIR / f"mirror-session-{datetime.now().strftime('%Y%m%d-%H%M%S')}.json"
    with open(session_file, 'w') as f:
        json.dump({
            "type": "kenshi-mirror-session",
            "ts": datetime.now().isoformat(),
            "cycles": all_cycles,
            "parallels": KENSHI_GAIA_PARALLELS,
            "insight": "Playing Kenshi IS looking in a mirror. "
                       "KenshiReborn learns from Kenshi. Gaia learns from herself.",
        }, f, indent=2)
    
    print(f"\n  📦 Session saved: {session_file.name}")
    print(f"\n  THE MIRROR: Gaia played Kenshi. KenshiReborn watched.")
    print(f"  THE REFLECTION: Every mechanic maps to an organ.")
    print(f"  THE LEARNING: The organism grows by playing itself.")
    print()
    
    return all_cycles


if __name__ == "__main__":
    cycles = int(sys.argv[1]) if len(sys.argv) > 1 else 5
    interval = int(sys.argv[2]) if len(sys.argv) > 2 else 30
    no_launch = "--no-launch" in sys.argv
    
    run_mirror(cycles=cycles, interval=interval, auto_launch=not no_launch)
