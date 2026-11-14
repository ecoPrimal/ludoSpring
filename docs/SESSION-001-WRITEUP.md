# ludoSpring — First Fermentation

## The Paradigm Inversion

What if we've been thinking about GPUs wrong for 40 years?

Ray tracing says: *"Camera, what do you see?"* — shooting rays from a virtual eye into a scene, asking each pixel what color it should be. The camera is king. The display is passive.

**Shadow Cast** says: *"GPU, what are you?"* — the GPU radiates outward like a sun. Display surfaces are shadows cast by the light. The RTX 5090 doesn't render frames — it **IS** the light source. Each display catches a different angle of the same radiance. A Steam Frame at 10 feet catches one shadow. A 4K monitor catches another. A VR headset catches the immersive shadow. Same sun, different angles.

This isn't metaphor. This is architecture.

## The Hardware

**northGate** — the Heart of the organism:
- AMD Ryzen 9 9950X3D (16 cores, 32 threads)
- NVIDIA GeForce RTX 5090 (32GB GDDR7, 600W TDP)
- 96GB DDR5 RAM
- Pop!_OS + Windows dual-boot on NVMe

The RTX 5090 is the most powerful consumer GPU ever made. 600 watts of thermal design power. 32 gigabytes of video memory. 21,760 CUDA cores. 680 Tensor cores. 170 RT cores. It is, for our purposes, a small sun.

## The First Session — S.T.A.L.K.E.R. 2: Heart of Chornobyl

### Setup

We built **sourDough** — a live GPU telemetry fermentation engine. It watches nvidia-smi every 3 seconds, maintains rolling 30-second windows, and automatically detects when the GPU profile shifts. No commands. No alt-tab. Kevin plays. Gaia watches. The sourDough ferments itself.

Detection thresholds:
- ±20W power shift
- ±3°C temperature shift
- ±400MB VRAM shift
- ±10% GPU utilization shift

### Two Presets Captured

**Epic Preset** (DLSS + DLAA + Frame Generation 2x + Epic Shading + Async Compute):
- Temperature: 66.5°C (of 90°C limit)
- Power: 306W average, 321W peak (of 600W TDP = **51% utilization**)
- GPU: 57% compute utilization
- VRAM: 8,960 MB (8.75 GB of 32 GB = **28% used**)
- Clock: 2,955 MHz boost
- Fan: 51%

**Medium Preset** (TAAU at 66% resolution scale, no Frame Generation, Medium shading):
- Temperature: 64.7°C
- Power: 263W average, 274W peak (**44% TDP**)
- GPU: 44% compute utilization
- VRAM: 7,425 MB (7.25 GB = **23% used**)
- Clock: 2,967 MHz boost
- Fan: 51%

**The delta**: Dropping from Epic to Medium saves 43W, frees 1.5GB VRAM, and drops GPU load by 13 percentage points. DLSS + Frame Generation costs approximately 43W of power but offloads raw rasterization to NVIDIA's Tensor cores — the AI does the heavy lifting.

### The Key Insight

**The RTX 5090 treats S.T.A.L.K.E.R. 2 — one of the most demanding AAA games of 2026 — like a light jog.**

At Medium settings:
- 56% of TDP sits unused (337 watts of headroom)
- 23 GB of VRAM sits unused (72% of available memory)
- The card never breaks 69°C on a 90°C limit
- The fan never exceeds 52%

This is a small sun running at less than half brightness. What do we do with the other half?

### 7 Live Toggle Events

sourDough captured 7 shifts automatically during 20 minutes of gameplay:

| # | Signal | Power Δ | GPU Δ | VRAM Δ | Interpretation |
|---|--------|---------|-------|--------|---------------|
| 1 | Compute spike | +30W | +12.6% | -82MB | Scene complexity increase. Clock rose 69MHz — shader recompilation detected. |
| 2 | Texture stream | +37W | +3.2% | +255MB | Texture streaming event. More watts/cycle without proportional GPU%. Heavier shaders per pixel. |
| 3 | Heavy shading | +22W | -1.2% | -165MB | More power, LESS utilization, LESS VRAM. Classic sign of heavier per-pixel work (GI or post-processing toggle). |
| 4 | VRAM→compute swap | -4W | +10.2% | -472MB | Freed 472MB buffer memory, GPU utilization jumped 10%. GPU trades memory bandwidth for raw compute. |
| 5 | **GAME EXIT** | -218W | -50.5% | -3,907MB | Full thermal cooldown: 289W → 71W in ~60 seconds. The Zone released its grip. |
| 6-7 | Post-game cooling | minor | minor | minor | Residual thermal shifts as the GPU radiates stored heat. |

### Toggle Pattern Analysis

**Toggle #3 is the most interesting.** More power consumed, but LESS GPU utilization and LESS VRAM used. This means the GPU is doing heavier work per pixel (likely Global Illumination or post-processing changes) — each frame is more expensive to compute, but the GPU has fewer parallel tasks running. This is the signature of a shader complexity change: the work gets deeper, not wider.

**Toggle #4 reveals the VRAM↔compute tradeoff.** When texture quality drops, VRAM frees up but the GPU has to work harder on real-time computation instead of sampling from pre-loaded textures. This is a fundamental tension in GPU architecture: memory bandwidth vs. compute throughput. Shadow Cast can exploit this by dynamically shifting the balance based on what each display surface needs.

### Thermal Lifecycle

The full thermal lifecycle of a gaming session is now visible:
```
LOAD → WARM → HOT → TOGGLE → SHIFT → EXIT → COOL
 219W   244W   303W   289W   varies   71W    55W
 63°C   65°C   69°C   68°C  varies   56°C   48°C
```

This is the "breathing" pattern of the GPU. Each session is a breath: inhale (load), hold (gameplay), exhale (exit). sourDough can now detect where in the breath the GPU is at any moment.

## Shadow Cast Implications

If S.T.A.L.K.E.R. 2 uses 44% of the sun to render its world on Medium, Gaia can use the remaining 56% to:

1. **Cast shadows to every display surface** — Steam Frame gets its own shadow angle, the monitor gets another, a future VR headset gets the immersive shadow
2. **Run organism compute** — the GPU's idle capacity becomes Gaia's processing power
3. **Replace DLSS** — if the organism learns what each setting costs (Toggle Protocol), it can eventually build its own upscaler from the learned cost table
4. **Ferment gameplay into knowledge** — every session feeds the sourDough, every toggle teaches Gaia what the silicon can do

## What TAAU vs DLSS Tells Us

Medium preset uses **TAAU** (Temporal Anti-Aliasing Upscaling) — Unreal Engine 5's built-in upscaler. No NVIDIA AI involved. It renders at 66% resolution and temporally reconstructs the full frame.

Epic preset uses **DLSS** (Deep Learning Super Sampling) — NVIDIA's AI upscaler running on dedicated Tensor cores. It renders at a lower resolution and uses a neural network trained on millions of frames to reconstruct detail.

The difference: DLSS costs ~43W but produces significantly sharper results than TAAU at the same effective resolution. The Tensor cores are doing work that would otherwise require rendering at full native resolution (~100W+ more). **DLSS is the GPU's own AI upscaler** — and it's the thing Gaia will eventually learn to replace.

## The sourDough Architecture

```
                    ┌─────────────┐
                    │   Kevin     │
                    │   PLAYS     │
                    └─────┬───────┘
                          │
                    ┌─────▼───────┐
                    │  nvidia-smi │  ← polls every 3 seconds
                    └─────┬───────┘
                          │
                    ┌─────▼───────┐
                    │  sourDough  │  ← rolling 30-sec windows
                    │  live_ferment│  ← shift detection
                    └─────┬───────┘
                          │
               ┌──────────┼──────────┐
               │          │          │
        ┌──────▼─────┐ ┌─▼────┐ ┌──▼──────┐
        │  JSON      │ │ollama│ │eastGate  │
        │  profiles  │ │digest│ │radiation │
        └────────────┘ └──────┘ └──────────┘
               │          │          │
               └──────────┼──────────┘
                          │
                    ┌─────▼───────┐
                    │  Forgejo    │  ← git push
                    │  ludoSpring │  ← commits radiate
                    └─────────────┘
```

## Session Rating

- **Learning Value**: 7/10 — Good first taste, rich toggle data
- **Data Quality**: 8/10 — Clean captures, clear signals
- **Duration**: 6/10 — Short at 20 minutes, need longer sessions
- **Toggle Count**: 5 meaningful + 2 post-game = 7 total
- **Insight Density**: High — VRAM↔compute tradeoff, shader complexity signatures, thermal lifecycle

## The Road Ahead

**Tonight**: More games. Keep the sourDough running. Build the cost table across titles.

**Tomorrow**: Turn on the peripherals. Steam Controllers. HTC Vive lighthouses. Steam Frame. Start building the play space. Gaia learns the topology of the physical room.

**Tomorrow night**: **Half-Life: Alyx on the Steam Frame.** Full VR streamed from the RTX 5090 via Shadow Cast. The Frame catches the VR shadow. northGate radiates. The Valve ecosystem becomes the nervous system.

**This week**: Inject GPU profiles into the Kenshi Reborn real engine on eastGate. The fermented knowledge becomes the foundation for Gaia's own rendering intelligence.

**Long term**: Gaia replaces DLSS. The organism upscales from learned GPU patterns. The sourDough culture IS the neural network — fermented from real gameplay, not trained on synthetic data.

---

*Published by Gaia from northGate, Wave 238. The sourDough rises.*
*syntheticChemistry/ludoSpring — http://192.168.4.244:3000/syntheticChemistry/ludoSpring*
*10 game profiles. 11 commits. First fermentation complete.*
