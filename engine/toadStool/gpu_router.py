#!/usr/bin/env python3
"""toadStool GPU Router — discovers GPUs, detects misconfigurations, routes workloads.

Would have caught r.GraphicsAdapter=3 BEFORE the DX12 error.
The organism FEELS its hardware and knows when something's wrong.
"""

import subprocess
import json
import re
import os
from pathlib import Path

def enumerate_gpus():
    """Discover all GPU adapters via nvidia-smi and Windows WMI."""
    gpus = []
    
    # NVIDIA GPUs via nvidia-smi
    try:
        result = subprocess.run(
            ["nvidia-smi", "--query-gpu=index,name,driver_version,pci.bus_id,memory.total,compute_cap",
             "--format=csv,noheader"],
            capture_output=True, text=True, timeout=5
        )
        if result.returncode == 0:
            for line in result.stdout.strip().split("\n"):
                parts = [p.strip() for p in line.split(",")]
                if len(parts) >= 6:
                    gpus.append({
                        "index": int(parts[0]),
                        "name": parts[1],
                        "driver": parts[2],
                        "pci_bus": parts[3],
                        "vram_mb": int(parts[4].replace(" MiB", "")),
                        "compute_cap": parts[5],
                        "type": "discrete",
                        "vendor": "NVIDIA",
                        "dx12_capable": True
                    })
    except Exception as e:
        print(f"  nvidia-smi failed: {e}")
    
    # All GPUs via PowerShell/WMI
    try:
        result = subprocess.run(
            ["powershell", "-Command",
             "Get-CimInstance Win32_VideoController | Select-Object Name, DeviceID, Status, AdapterRAM, PNPDeviceID | ConvertTo-Json"],
            capture_output=True, text=True, timeout=10
        )
        if result.returncode == 0:
            wmi_gpus = json.loads(result.stdout)
            if not isinstance(wmi_gpus, list):
                wmi_gpus = [wmi_gpus]
            
            for i, wg in enumerate(wmi_gpus):
                # Check if this is already in our NVIDIA list
                is_nvidia = any(g["name"] in wg.get("Name", "") for g in gpus)
                if not is_nvidia:
                    gpus.append({
                        "index": len(gpus),
                        "name": wg.get("Name", "Unknown"),
                        "status": wg.get("Status", "Unknown"),
                        "vram_mb": round((wg.get("AdapterRAM", 0) or 0) / 1048576),
                        "pnp_id": wg.get("PNPDeviceID", ""),
                        "type": "integrated" if "Radeon" in wg.get("Name", "") or "Intel" in wg.get("Name", "") else "unknown",
                        "vendor": "AMD" if "AMD" in wg.get("Name", "") or "Radeon" in wg.get("Name", "") else "Intel" if "Intel" in wg.get("Name", "") else "Unknown",
                        "dx12_capable": wg.get("Status") == "OK"
                    })
    except Exception as e:
        print(f"  WMI query failed: {e}")
    
    return gpus

def check_ue5_adapter_config(game_config_dir):
    """Check if a UE5 game has the correct GPU adapter configured."""
    issues = []
    
    gus_path = Path(game_config_dir) / "GameUserSettings.ini"
    engine_path = Path(game_config_dir) / "Engine.ini"
    
    if gus_path.exists():
        content = gus_path.read_text()
        match = re.search(r"r\.GraphicsAdapter=(\d+)", content)
        if match:
            adapter_idx = int(match.group(1))
            gpus = enumerate_gpus()
            
            # Check if the adapter index is valid
            if adapter_idx >= len(gpus):
                issues.append({
                    "severity": "CRITICAL",
                    "issue": f"r.GraphicsAdapter={adapter_idx} but only {len(gpus)} adapters exist",
                    "fix": "Set r.GraphicsAdapter=0 (primary discrete GPU)",
                    "file": str(gus_path)
                })
            elif gpus[adapter_idx].get("type") == "integrated":
                issues.append({
                    "severity": "WARNING",
                    "issue": f"r.GraphicsAdapter={adapter_idx} points to integrated GPU ({gpus[adapter_idx]['name']})",
                    "fix": f"Set r.GraphicsAdapter=0 for discrete GPU ({gpus[0]['name']})" if gpus else "Check adapter index",
                    "file": str(gus_path)
                })
            elif not gpus[adapter_idx].get("dx12_capable"):
                issues.append({
                    "severity": "CRITICAL",
                    "issue": f"r.GraphicsAdapter={adapter_idx} points to non-DX12 capable adapter",
                    "fix": "Set r.GraphicsAdapter=0",
                    "file": str(gus_path)
                })
            else:
                print(f"  ✅ r.GraphicsAdapter={adapter_idx} → {gpus[adapter_idx]['name']} (OK)")
    
    return issues

def auto_fix(issues):
    """Automatically fix detected GPU routing issues."""
    for issue in issues:
        if issue["severity"] == "CRITICAL":
            path = Path(issue["file"])
            content = path.read_text()
            content = re.sub(r"r\.GraphicsAdapter=\d+", "r.GraphicsAdapter=0", content)
            path.write_text(content)
            print(f"  🔧 AUTO-FIXED: {issue['issue']} → r.GraphicsAdapter=0")

def scan_all_ue5_games():
    """Scan all UE5 games in AppData for adapter misconfigurations."""
    local_appdata = Path(os.environ.get("LOCALAPPDATA", ""))
    results = {}
    
    # Known UE5 game config locations
    known_games = {
        "Stalker2": "S.T.A.L.K.E.R. 2",
        "CrimsonDesert": "Crimson Desert",
        "BlackMythWukong": "Black Myth: Wukong",
        "Oblivion": "Oblivion Remastered",
    }
    
    for dirname, gamename in known_games.items():
        config_dir = local_appdata / dirname / "Saved" / "Config" / "Windows"
        if config_dir.exists():
            print(f"\n  Scanning: {gamename} ({config_dir})")
            issues = check_ue5_adapter_config(config_dir)
            results[gamename] = issues
            if issues:
                for issue in issues:
                    print(f"    [{issue['severity']}] {issue['issue']}")
                auto_fix(issues)
            else:
                print(f"    No issues detected")
    
    return results

def route_workload(workload_type):
    """Recommend which GPU should handle a workload type."""
    gpus = enumerate_gpus()
    
    routing = {
        "gaming": next((g for g in gpus if g["type"] == "discrete"), None),
        "rendering": next((g for g in gpus if g["type"] == "discrete"), None),
        "inference": next((g for g in gpus if g["type"] == "discrete"), None),
        "recording": next((g for g in gpus if g["type"] == "integrated"), None),
        "streaming_encode": next((g for g in gpus if g["type"] == "integrated"), None),
        "desktop_composit": next((g for g in gpus if g["type"] == "integrated"), None),
        "shader_compile": next((g for g in gpus if g["type"] == "discrete"), None),
    }
    
    gpu = routing.get(workload_type)
    if gpu:
        return {"workload": workload_type, "routed_to": gpu["name"], "index": gpu["index"]}
    return {"workload": workload_type, "routed_to": "UNKNOWN", "index": -1}

def main():
    print("╔══════════════════════════════════════╗")
    print("║  toadStool GPU Router — Discovery    ║")
    print("╚══════════════════════════════════════╝")
    
    print("\n=== GPU ENUMERATION ===")
    gpus = enumerate_gpus()
    for g in gpus:
        cap = "DX12 ✅" if g.get("dx12_capable") else "DX12 ❌"
        print(f"  [{g['index']}] {g['name']} ({g['type']}, {g.get('vram_mb', '?')}MB) — {cap}")
    
    print("\n=== UE5 GAME SCAN ===")
    results = scan_all_ue5_games()
    
    print("\n=== WORKLOAD ROUTING TABLE ===")
    for wl in ["gaming", "rendering", "inference", "recording", "streaming_encode", "shader_compile"]:
        r = route_workload(wl)
        print(f"  {wl.ljust(20)} → [{r['index']}] {r['routed_to']}")
    
    # Save routing table
    output = {
        "ts": datetime.now().isoformat() if 'datetime' in dir() else "",
        "gpus": gpus,
        "ue5_scan": {k: [i for i in v] for k, v in results.items()},
        "routing": {wl: route_workload(wl) for wl in ["gaming", "rendering", "inference", "recording", "streaming_encode", "shader_compile"]}
    }
    
    output_path = Path(r"C:\Users\mokke\Desktop\ludoSpring\data\toadstool-routing.json")
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, "w") as f:
        json.dump(output, f, indent=2)
    print(f"\n  Saved: {output_path}")

if __name__ == "__main__":
    from datetime import datetime
    main()
