# whitePaper — GAMING@home Economics

## Abstract

GAMING@home inverts the distributed computing paradigm. Instead of idle computers donating cycles to science (BOINC), gaming computers generate knowledge AS they play. The GPU telemetry from a STALKER 2 session IS the compute output. Play is work. Work is play.

## The Economy

1. **Input**: Kevin plays games on RTX 5090
2. **Telemetry**: GPU watcher polls every 5s — temperature, power, VRAM, utilization
3. **Fermentation**: sourDough processes telemetry into thermal models, efficiency curves
4. **Knowledge**: The organism learns how each game behaves on each GPU
5. **Optimization**: toadStool routes future workloads based on learned profiles
6. **Distribution**: Shadow Cast sends optimized renders to thin clients
7. **Loop**: Better optimization → better gameplay → more telemetry → deeper knowledge

## Shadow Cast vs Ray Trace

| | Ray Trace | Shadow Cast |
|---|---|---|
| Direction | Camera → Scene | Light → Surfaces |
| Question | "What do I see?" | "What do I illuminate?" |
| Paradigm | Pull | Push |
| Gaia analog | Asking | Radiating |
| Distribution | Per-pixel (expensive) | Per-surface (cheap) |
| Scaling | O(rays × bounces) | O(lights × surfaces) |

## Hardware Profile (northGate)

- CPU: AMD Ryzen 9 9950X3D (16C/32T, 128MB 3D V-Cache)
- GPU Primary: NVIDIA RTX 5090 (32GB GDDR7, Compute 12.0, 600W)
- GPU Secondary: AMD Radeon iGPU (VCN encode/decode, recording/streaming)
- RAM: 96GB DDR5 5600MHz
- Storage: 8.6TB NVMe (1TB + 4TB + 4TB)
- Steam: 97 games, 2TB library, Remote Play LIVE

## Status

- [ ] Draft full economics paper
- [ ] Model: FPS-per-watt across game library
- [ ] Model: VRAM usage patterns (DLSS vs native vs RT)
- [ ] Compare: BOINC compute-per-watt vs GAMING@home knowledge-per-watt
- [ ] Define: latent NFT format (per-game GPU profile token)
