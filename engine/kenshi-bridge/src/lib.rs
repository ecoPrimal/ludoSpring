// KenshiReborn Bridge DLL — RenderSystem_Vulkan_x64.dll
// Phase 1: Register with Ogre's Root to appear in the launcher dropdown
//
// Kenshi uses Ogre 2.x. When the DLL is loaded, Ogre calls dllStartPlugin().
// We get Root::getSingletonPtr(), construct a base RenderSystem, set its name,
// and register it with Root::addRenderSystem().

#![allow(non_snake_case)]

use std::ffi::CStr;
use std::fs::OpenOptions;
use std::io::Write;
use std::ptr;

const LOG_FILE: &str = r"M:\SteamLibrary\steamapps\common\Kenshi\kenshi_reborn.log";
const PROOF_FILE: &str = r"M:\SteamLibrary\steamapps\common\Kenshi\kenshi_reborn_loaded.txt";

fn log(msg: &str) {
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(LOG_FILE) {
        let _ = writeln!(f, "{}", msg);
    }
}

// Windows FFI
extern "system" {
    fn LoadLibraryA(name: *const u8) -> *mut u8;
    fn GetProcAddress(module: *mut u8, name: *const u8) -> *mut u8;
    fn GetLastError() -> u32;
}

// Ogre function signatures (x64 MSVC: thiscall = normal calling convention)
type FnGetSingletonPtr = unsafe extern "C" fn() -> *mut u8;
type FnAddRenderSystem = unsafe extern "C" fn(root: *mut u8, rs: *mut u8);
type FnRenderSystemCtor = unsafe extern "C" fn(this: *mut u8);

// MSVC mangled symbol names from dumpbin /exports OgreMain_x64.dll
const SYM_ROOT_GET_SINGLETON_PTR: &[u8] = b"?getSingletonPtr@Root@Ogre@@SAPEAV12@XZ\0";
const SYM_ROOT_ADD_RENDER_SYSTEM: &[u8] = b"?addRenderSystem@Root@Ogre@@QEAAXPEAVRenderSystem@2@@Z\0";
const SYM_RENDER_SYSTEM_CTOR: &[u8] = b"??0RenderSystem@Ogre@@QEAA@XZ\0";

// RenderSystem name — must fit in MSVC SSO (max 15 chars + null)
// "Vulkan (Reborn)" = exactly 15 chars — PERFECT SSO fit
const RS_NAME: &[u8] = b"Vulkan (Reborn)\0";
const RS_NAME_LEN: usize = 15;

/// Write an MSVC std::string in SSO mode at the given memory address.
/// MSVC std::string layout (32 bytes total):
///   [0..15]  _Buf[16]  — SSO char buffer (or _Ptr + padding for heap)
///   [16..23] _Mysize   — string length (size_t)
///   [24..31] _Myres    — capacity, 15 for SSO (size_t)
unsafe fn write_msvc_string_sso(addr: *mut u8, s: &[u8], len: usize) {
    // Write string data into SSO buffer
    ptr::copy_nonoverlapping(s.as_ptr(), addr, len + 1); // include null terminator
    // Zero remaining buffer bytes
    for i in (len + 1)..16 {
        *addr.add(i) = 0;
    }
    // Write size
    let size_ptr = addr.add(16) as *mut u64;
    *size_ptr = len as u64;
    // Write capacity = 15 (SSO marker)
    let cap_ptr = addr.add(24) as *mut u64;
    *cap_ptr = 15u64;
}

/// Ogre3D plugin entry point — called when Kenshi loads this DLL
#[unsafe(no_mangle)]
pub extern "C" fn dllStartPlugin() {
    log("╔═══════════════════════════════════════════════════════╗");
    log("║  KenshiReborn v0.5.0 — DROPDOWN INJECTION            ║");
    log("╚═══════════════════════════════════════════════════════╝");
    
    unsafe {
        // Step 1: Get OgreMain module handle
        let ogre = LoadLibraryA(b"OgreMain_x64.dll\0".as_ptr());
        if ogre.is_null() {
            log("ERROR: Could not load OgreMain_x64.dll");
            return;
        }
        log("✓ OgreMain_x64.dll loaded");
        
        // Step 2: Resolve symbols
        let fn_get_root = GetProcAddress(ogre, SYM_ROOT_GET_SINGLETON_PTR.as_ptr());
        let fn_add_rs = GetProcAddress(ogre, SYM_ROOT_ADD_RENDER_SYSTEM.as_ptr());
        let fn_rs_ctor = GetProcAddress(ogre, SYM_RENDER_SYSTEM_CTOR.as_ptr());
        
        if fn_get_root.is_null() || fn_add_rs.is_null() || fn_rs_ctor.is_null() {
            log(&format!("ERROR: Symbol resolution failed"));
            log(&format!("  getSingletonPtr: {:?}", fn_get_root));
            log(&format!("  addRenderSystem: {:?}", fn_add_rs));
            log(&format!("  RenderSystem ctor: {:?}", fn_rs_ctor));
            write_proof("PARTIAL — DLL loaded but Ogre symbols not found");
            return;
        }
        log("✓ All Ogre symbols resolved");
        
        let get_root: FnGetSingletonPtr = std::mem::transmute(fn_get_root);
        let add_rs: FnAddRenderSystem = std::mem::transmute(fn_add_rs);
        let rs_ctor: FnRenderSystemCtor = std::mem::transmute(fn_rs_ctor);
        
        // Step 3: Get Root singleton
        let root = get_root();
        if root.is_null() {
            log("ERROR: Root::getSingletonPtr() returned null");
            write_proof("PARTIAL — Ogre Root not yet initialized");
            return;
        }
        log(&format!("✓ Root singleton at {:?}", root));
        
        // Step 4: Allocate memory for our RenderSystem (oversized for safety)
        let rs_size = 8192;
        let layout = std::alloc::Layout::from_size_align(rs_size, 64).unwrap();
        let rs = std::alloc::alloc_zeroed(layout);
        if rs.is_null() {
            log("ERROR: Failed to allocate RenderSystem memory");
            return;
        }
        log(&format!("✓ Allocated {} bytes for RenderSystem at {:?}", rs_size, rs));
        
        // Step 5: Call the base RenderSystem constructor
        log("  Calling RenderSystem::RenderSystem()...");
        rs_ctor(rs);
        log("✓ RenderSystem constructed");
        
        // Step 6: Set mName
        // In Ogre's RenderSystem, mName is the first std::string member after the vtable pointer.
        // Layout: [vtable_ptr: 8 bytes] [mName: 32 bytes (MSVC std::string)] ...
        // mName offset = 8 bytes
        let name_offset = 8usize;
        let name_addr = rs.add(name_offset);
        
        log(&format!("  Writing name at offset {} (addr {:?})", name_offset, name_addr));
        write_msvc_string_sso(name_addr, RS_NAME, RS_NAME_LEN);
        log(&format!("✓ Name set to '{}'", std::str::from_utf8(&RS_NAME[..RS_NAME_LEN]).unwrap()));
        
        // Step 7: Register with Root
        log("  Calling Root::addRenderSystem()...");
        add_rs(root, rs);
        log("✓ RenderSystem registered with Ogre Root!");
        log("");
        log("╔═══════════════════════════════════════════════════════╗");
        log("║  SUCCESS — 'Vulkan (Reborn)' should appear in the    ║");
        log("║  Kenshi launcher dropdown!                            ║");
        log("╚═══════════════════════════════════════════════════════╝");
        
        write_proof("SUCCESS — RenderSystem registered with Ogre Root. Check the dropdown!");
    }
}

fn write_proof(status: &str) {
    if let Ok(mut f) = std::fs::File::create(PROOF_FILE) {
        let _ = writeln!(f, "KenshiReborn v0.5.0 — Bridge DLL");
        let _ = writeln!(f, "Status: {}", status);
        let _ = writeln!(f, "");
        let _ = writeln!(f, "If 'Vulkan (Reborn)' appears in the dropdown — PHASE 1 COMPLETE!");
        let _ = writeln!(f, "Selecting it won't render yet (Phase 2), but the injection path is proven.");
    }
}

/// Ogre3D plugin shutdown — called when Kenshi exits
#[unsafe(no_mangle)]
pub extern "C" fn dllStopPlugin() {
    log("dllStopPlugin() called — clean shutdown");
}

/// DLL main entry point
#[unsafe(no_mangle)]
pub extern "system" fn DllMain(
    _hinst: usize,
    reason: u32,
    _reserved: *mut std::ffi::c_void,
) -> i32 {
    if reason == 1 {
        log("DllMain: PROCESS_ATTACH — mapped into kenshi_x64.exe");
    }
    1
}
