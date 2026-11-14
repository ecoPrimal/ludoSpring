# post-reboot-discovery.ps1 — Find EVERYTHING on northGate + LAN
# Run after reboot to map all connectable devices for ludoSpring

Write-Host "╔══════════════════════════════════════════════╗"
Write-Host "║  ludoSpring POST-REBOOT DISCOVERY            ║"
Write-Host "║  Working BACKWARDS from hardware             ║"
Write-Host "╚══════════════════════════════════════════════╝"
Write-Host ""

# === 1. LOCAL GPU STATE (did reboot fix DX12?) ===
Write-Host "=== [1] GPU STATE ==="
Get-PnpDevice -Class Display | Select-Object FriendlyName, Status | Format-Table -AutoSize
nvidia-smi --query-gpu=name,temperature.gpu,power.draw,memory.used,memory.total --format=csv,noheader 2>$null
Write-Host ""

# === 2. BLUETOOTH DEVICES ===
Write-Host "=== [2] BLUETOOTH ==="
Get-PnpDevice -Class Bluetooth -ErrorAction SilentlyContinue | Select-Object FriendlyName, Status | Format-Table -AutoSize
Get-PnpDevice | Where-Object { $_.Class -eq "BTHLEDevice" -or $_.FriendlyName -match "Steam|Corsair|Vive|Controller" } | Select-Object FriendlyName, Status | Format-Table -AutoSize
Write-Host ""

# === 3. USB PERIPHERALS ===
Write-Host "=== [3] USB DEVICES ==="
Get-PnpDevice -Class USB -Status OK | Where-Object { $_.FriendlyName -notmatch "Hub|Root|Host Controller" } | Select-Object FriendlyName, InstanceId | Format-Table -AutoSize
Write-Host ""

# === 4. STEAM DEVICES ===
Write-Host "=== [4] STEAM ECOSYSTEM ==="
Get-PnpDevice | Where-Object { $_.FriendlyName -match "Steam|Valve|28de" -or $_.InstanceId -match "28DE" } | Select-Object FriendlyName, Status, InstanceId | Format-Table -AutoSize
# Check Steam streaming
netstat -an | Select-String "27036|27037|27031|27032|27033|27034|27035" | Select-Object -First 10
Write-Host ""

# === 5. LAN SCAN — Find Steam Frame, Controllers, all devices ===
Write-Host "=== [5] LAN SCAN (192.168.4.0/24) ==="
Write-Host "  Scanning... (this takes ~30 sec)"
$jobs = @()
1..254 | ForEach-Object {
    $ip = "192.168.4.$_"
    $jobs += Start-Job -ScriptBlock {
        param($ip)
        if (Test-Connection $ip -Count 1 -TimeoutSeconds 1 -Quiet) {
            try { $hostname = [System.Net.Dns]::GetHostEntry($ip).HostName } catch { $hostname = "?" }
            "$ip|$hostname|UP"
        }
    } -ArgumentList $ip
}
$results = $jobs | Wait-Job -Timeout 30 | Receive-Job
$jobs | Remove-Job -Force -ErrorAction SilentlyContinue
$results | Sort-Object | ForEach-Object {
    $parts = $_.Split("|")
    Write-Host "  $($parts[0].PadRight(18)) $($parts[1].PadRight(30)) $($parts[2])"
}
Write-Host ""

# === 6. LAN 2 SCAN (192.168.9.0/24 — House 2) ===
Write-Host "=== [6] LAN 2 SCAN (192.168.9.0/24 — House 2) ==="
@("192.168.9.1", "192.168.9.111", "192.168.9.203", "192.168.9.217") | ForEach-Object {
    $up = Test-Connection $_ -Count 1 -TimeoutSeconds 2 -Quiet
    Write-Host "  $($_.PadRight(18)) $(if($up){'UP'}else{'DOWN'})"
}
Write-Host ""

# === 7. STEAM REMOTE PLAY CLIENTS ===
Write-Host "=== [7] STEAM REMOTE PLAY ==="
$steamConfig = "C:\Program Files (x86)\Steam\config\config.vdf"
if (Test-Path $steamConfig) {
    Select-String -Path $steamConfig -Pattern "streaming|authorized|device|remote" -Context 0,2 | Select-Object -First 10 | ForEach-Object { Write-Host "  $($_.Line.Trim())" }
}
Write-Host ""

# === 8. FLINT ROUTER USB DEVICES ===
Write-Host "=== [8] FLINT ROUTER (USB devices + signals) ==="
Write-Host "  SSH to router to check USB..."
echo "ExoticPanda714!!" | ssh -o StrictHostKeyChecking=no -o ConnectTimeout=5 root@192.168.4.1 "echo '=== USB ==='; lsusb 2>/dev/null || cat /sys/kernel/debug/usb/devices 2>/dev/null | head -60; echo '=== MOUNTS ==='; mount | grep usb; echo '=== VIDEO ==='; ls -la /dev/video* 2>/dev/null; echo '=== AUDIO ==='; arecord -l 2>/dev/null; aplay -l 2>/dev/null" 2>&1
Write-Host ""

# === 9. AUDIO ENDPOINTS ===
Write-Host "=== [9] AUDIO ==="
Get-CimInstance Win32_SoundDevice | Select-Object Name, Status | Format-Table -AutoSize
Write-Host ""

# === 10. SUMMARY ===
Write-Host "╔══════════════════════════════════════════════╗"
Write-Host "║  DISCOVERY COMPLETE — Map all connections    ║"
Write-Host "║  Steam Frame? Controllers? BT headset?       ║"
Write-Host "║  Flint USB relay? Valve devices?             ║"
Write-Host "║  Work BACKWARDS to ludoSpring                ║"
Write-Host "╚══════════════════════════════════════════════╝"
