// KenshiReborn Bridge DLL — RenderSystem_Vulkan_x64.dll
// v0.7.5 — DIAGNOSTIC: no-op DLL to isolate crash
//
// sourDough log:
//   v0.7.0-v0.7.4: all crash after getConfigOptions/setConfigOption
//   v0.7.5: PURE NO-OP — does NOT register any RenderSystem
//           If Kenshi runs → our fake RS was the problem
//           If Kenshi crashes → Ogre plugin loading itself is broken

#![allow(non_snake_case, unused)]

use std::fs::OpenOptions;
use std::io::Write;

const LOG_FILE: &str = r"M:\SteamLibrary\steamapps\common\Kenshi\kenshi_reborn.log";

fn log(msg: &str) {
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(LOG_FILE) {
        let _ = writeln!(f, "{}", msg);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dllStartPlugin() {
    let _ = std::fs::File::create(LOG_FILE);
    log("╔═══════════════════════════════════════════════════════╗");
    log("║  KenshiReborn v0.7.5 — DIAGNOSTIC NO-OP             ║");
    log("║  Pure observer. Registers NOTHING. Just watches.     ║");
    log("╚═══════════════════════════════════════════════════════╝");
    log("  If you see this and Kenshi runs: fake RS was the bug.");
    log("  If Kenshi still crashes: Ogre plugin load is broken.");
}

#[unsafe(no_mangle)]
pub extern "C" fn dllStopPlugin() {
    log("  dllStopPlugin() — observer standing down");
}
