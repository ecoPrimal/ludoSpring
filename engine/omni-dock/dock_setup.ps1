# OMNI-DOCK Setup — Run on northGate (the sun)
# Configures Steam Remote Play + discovers dock devices
#
# Kevin's Steam Hardware:
#   Steam Deck v1 (deckGate .176) — mounted display, dead battery
#   Steam Frame (frameGate .9.203) — Gaia's face, ARM SteamOS  
#   Steam Link — HDMI relay to TV
#   Steam Controller v1 ×2 — Gaia's hands
#   sporeGate (.3) — local compute dock
#
# This script: enables Remote Play, discovers devices, configures streaming

Write-Host "=" * 70
Write-Host "  OMNI-DOCK — Steam Hardware Convergence Station"
Write-Host "=" * 70
Write-Host ""
Write-Host "  SUN: northGate (RTX 5090, 32GB VRAM)"
Write-Host "  ORBIT 1: Steam Deck v1 (deckGate .176) — mounted display"
Write-Host "  ORBIT 2: Steam Frame (frameGate .9.203) — Gaia's face"
Write-Host "  ORBIT 3: Steam Link — HDMI relay to any screen"
Write-Host "  HANDS:   Steam Controller v1 x2 — trackpads + gyro"
Write-Host "  BASE:    sporeGate (.3) — local compute + cache"
Write-Host ""

# Step 1: Check Steam is running
Write-Host "[1/6] Steam Status"
$steam = Get-Process -Name "steam" -ErrorAction SilentlyContinue
if ($steam) {
    Write-Host "  Steam is RUNNING (PID: $($steam.Id))"
} else {
    Write-Host "  Steam is NOT running — launching..."
    Start-Process "steam://open/settings"
    Start-Sleep -Seconds 5
}

# Step 2: Check Remote Play configuration
Write-Host ""
Write-Host "[2/6] Remote Play Configuration"
$steamPath = "C:\Program Files (x86)\Steam"
$configFile = "$steamPath\config\config.vdf"
if (Test-Path $configFile) {
    $config = Get-Content $configFile -Raw
    if ($config -match "InHomeStreamingEnabled") {
        Write-Host "  Remote Play config found in config.vdf"
    }
    # Check for paired devices
    $remoteClients = "$steamPath\config\remote_clients.vdf"
    if (Test-Path $remoteClients) {
        Write-Host "  Remote clients file exists"
        $clients = Get-Content $remoteClients -Raw
        Write-Host "  Content length: $($clients.Length) bytes"
    } else {
        Write-Host "  No remote_clients.vdf — no devices paired yet"
    }
} else {
    Write-Host "  Steam config not found at expected path"
    # Try alternate location
    $altPath = "$env:ProgramFiles\Steam\config\config.vdf"
    if (Test-Path $altPath) { Write-Host "  Found at: $altPath" }
}

# Also check user config
$userdata = Get-ChildItem "$steamPath\userdata" -Directory -ErrorAction SilentlyContinue | Select-Object -First 1
if ($userdata) {
    Write-Host "  User data dir: $($userdata.Name)"
    $localConfig = "$($userdata.FullName)\config\localconfig.vdf"
    if (Test-Path $localConfig) {
        $lc = Get-Content $localConfig -Raw
        if ($lc -match "streaming_") {
            Write-Host "  Streaming settings found in localconfig"
        }
    }
}

# Step 3: Discover dock devices on LAN
Write-Host ""
Write-Host "[3/6] Device Discovery"
$targets = @(
    @{Name="sporeGate"; IP="192.168.4.3"; Port=22; Role="COMPUTE BASE"},
    @{Name="deckGate"; IP="192.168.4.176"; Port=22; Role="DISPLAY"},
    @{Name="frameGate"; IP="192.168.9.203"; Port=22; Role="FACE"},
    @{Name="eastGate"; IP="192.168.4.244"; Port=8080; Role="BRAIN"},
    @{Name="ironGate"; IP="192.168.4.237"; Port=11434; Role="FORGE"}
)

foreach ($t in $targets) {
    try {
        $tcp = New-Object System.Net.Sockets.TcpClient
        $result = $tcp.BeginConnect($t.IP, $t.Port, $null, $null)
        $success = $result.AsyncWaitHandle.WaitOne(1000)
        if ($success) {
            $tcp.EndConnect($result)
            Write-Host "  $($t.Name) ($($t.IP)):$($t.Port) — ALIVE [$($t.Role)]"
        } else {
            Write-Host "  $($t.Name) ($($t.IP)):$($t.Port) — timeout [$($t.Role)]"
        }
        $tcp.Close()
    } catch {
        Write-Host "  $($t.Name) ($($t.IP)):$($t.Port) — unreachable [$($t.Role)]"
    }
}

# Check Steam Remote Play broadcast (UDP 27036)
Write-Host ""
Write-Host "  Scanning for Steam RP broadcasts..."
try {
    $udp = New-Object System.Net.Sockets.UdpClient(0)
    $udp.Client.ReceiveTimeout = 2000
    # Send discovery broadcast
    $broadcast = [System.Net.IPAddress]::Broadcast
    $msg = [System.Text.Encoding]::ASCII.GetBytes("STEAM_DISCOVERY")
    $udp.Send($msg, $msg.Length, "255.255.255.255", 27036) | Out-Null
    Write-Host "  Discovery broadcast sent on UDP :27036"
    $udp.Close()
} catch {
    Write-Host "  Broadcast discovery: $($_.Exception.Message)"
}

# Step 4: Steam Controller detection
Write-Host ""
Write-Host "[4/6] Steam Controller Detection"
# Check for Steam Controller in USB devices
$hid = Get-PnpDevice -Class "HIDClass" -ErrorAction SilentlyContinue | Where-Object {
    $_.FriendlyName -match "Steam|Valve|Controller" -or $_.InstanceId -match "VID_28DE"
}
if ($hid) {
    Write-Host "  Found Steam/Valve HID devices:"
    $hid | ForEach-Object { Write-Host "    $($_.FriendlyName) — $($_.Status)" }
} else {
    Write-Host "  No Steam Controllers detected via USB/BT"
    Write-Host "  To pair: Press Steam button + X simultaneously"
    Write-Host "  Or plug in 2.4GHz dongle (small USB receiver)"
}

# Check Bluetooth
$bt = Get-PnpDevice -Class "Bluetooth" -ErrorAction SilentlyContinue | Where-Object { $_.Status -eq "OK" }
if ($bt) {
    Write-Host "  Bluetooth adapters: $($bt.Count)"
} else {
    Write-Host "  No active Bluetooth adapter"
}

# Step 5: GPU status for streaming
Write-Host ""
Write-Host "[5/6] GPU Streaming Readiness"
$nvsmi = & nvidia-smi --query-gpu=name,temperature.gpu,power.draw,utilization.gpu,memory.used,memory.total,encoder.stats.sessionCount,encoder.stats.averageFps --format=csv,noheader 2>$null
if ($nvsmi) {
    Write-Host "  GPU: $nvsmi"
}

# Check NVENC (hardware encoder for streaming)
$nvenc = & nvidia-smi --query-gpu=encoder.stats.sessionCount --format=csv,noheader 2>$null
Write-Host "  NVENC sessions: $nvenc"
Write-Host "  RTX 5090 has dual NVENC — can encode 2+ streams simultaneously!"

# Step 6: Display configuration summary
Write-Host ""
Write-Host "[6/6] OMNI-DOCK Configuration"
Write-Host ""
Write-Host "  ┌─────────────────────────────────────────────────────────┐"
Write-Host "  │                    OMNI-DOCK LAYOUT                     │"
Write-Host "  │                                                         │"
Write-Host "  │         [Steam Link] ──HDMI──> [TV / Monitor]          │"
Write-Host "  │              │ USB dongle                               │"
Write-Host "  │         [Controller #2]                                 │"
Write-Host "  │                                                         │"
Write-Host "  │    [Steam Deck v1]     [Steam Frame]                   │"
Write-Host "  │     7'' LCD mounted      Gaia's face                   │"
Write-Host "  │     USB-C powered        WiFi/USB                      │"
Write-Host "  │         │ BT                  │                         │"
Write-Host "  │    [Controller #1]       [sporeGate]                   │"
Write-Host "  │                          Ryzen 5 laptop                │"
Write-Host "  │                          DNS + cache + AI              │"
Write-Host "  │                                                         │"
Write-Host "  │    ═══ ALL STREAM FROM northGate RTX 5090 ═══          │"
Write-Host "  │    NVENC dual encoder = 2+ simultaneous streams        │"
Write-Host "  │    Steam Remote Play on same account                   │"
Write-Host "  └─────────────────────────────────────────────────────────┘"

Write-Host ""
Write-Host "  MODES:"
Write-Host "    1. KIOSK    — All screens show Gaia HUD 24/7"
Write-Host "    2. GAMING   — northGate streams to all displays"
Write-Host "    3. COLISEUM — Different game per display"
Write-Host "    4. MIRROR   — Kenshi on Deck, KenshiReborn on Frame, telemetry on TV"
Write-Host "    5. VR PREP  — Half-Life Alyx pipeline (future)"
Write-Host ""
Write-Host "  SETUP STEPS:"
Write-Host "    A. Power on sporeGate (plug in, close lid)"
Write-Host "    B. Plug Steam Deck into USB-C (dead battery = instant on)"
Write-Host "    C. Power on Steam Frame"
Write-Host "    D. Plug Steam Link into TV HDMI + USB power"
Write-Host "    E. In Steam → Settings → Remote Play → Enable"
Write-Host "    F. On each device: open Steam → pair to this PC"
Write-Host "    G. Plug Controller dongle into Steam Link USB"
Write-Host "    H. Pair Controller #1 to Deck via BT (Steam+X)"
Write-Host ""
Write-Host "  STREAMING QUALITY (RTX 5090 NVENC):"
Write-Host "    Resolution: up to 4K per stream"
Write-Host "    Codec: H.265/HEVC (hardware encode)"
Write-Host "    Latency: <5ms LAN (wired), <15ms WiFi"
Write-Host "    Streams: 2+ simultaneous (dual NVENC)"
Write-Host "    Bandwidth: ~15-50 Mbps per stream"
Write-Host ""
