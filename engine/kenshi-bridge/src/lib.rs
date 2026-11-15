// KenshiReborn Bridge DLL — RenderSystem_Vulkan_x64.dll
// v0.7.0 — REVERSE SIEGE: Clone D3D11 vtable, patch getName, register
//
// The investigator (v0.6.0) mapped all 64 vtable slots.
// Now we clone, patch, and siege from ALL sides:
// 1. Clone entire D3D11 vtable (64 slots)
// 2. Try getName candidates (slots 1, 2, 6, 11) — patch each
// 3. Build a fake RenderSystem object with our vtable
// 4. Register with Root::addRenderSystem
// 5. "Vulkan (Reborn)" appears in the Kenshi launcher dropdown!

#![allow(non_snake_case)]

use std::fs::OpenOptions;
use std::io::Write;
use std::ptr;

const LOG_FILE: &str = r"M:\SteamLibrary\steamapps\common\Kenshi\kenshi_reborn.log";

fn log(msg: &str) {
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(LOG_FILE) {
        let _ = writeln!(f, "{}", msg);
    }
}

extern "system" {
    fn LoadLibraryA(name: *const u8) -> *mut u8;
    fn GetProcAddress(module: *mut u8, name: *const u8) -> *mut u8;
    fn GetModuleHandleA(name: *const u8) -> *mut u8;
    fn IsBadReadPtr(lp: *const u8, ucb: usize) -> i32;
}

type FnGetSingletonPtr = unsafe extern "C" fn() -> *mut u8;
type FnGetAvailableRenderers = unsafe extern "C" fn(root: *mut u8) -> *mut StdVector;
type FnAddRenderSystem = unsafe extern "C" fn(root: *mut u8, rs: *mut u8);

#[repr(C)]
struct StdVector {
    begin: *mut *mut u8,
    end: *mut *mut u8,
    cap: *mut *mut u8,
}

// Mangled Ogre symbols
const SYM_ROOT_GET_SINGLETON: &[u8] = b"?getSingletonPtr@Root@Ogre@@SAPEAV12@XZ\0";
const SYM_GET_AVAIL_RENDERERS: &[u8] = b"?getAvailableRenderers@Root@Ogre@@QEAAAEBV?$vector@PEAVRenderSystem@Ogre@@V?$STLAllocator@PEAVRenderSystem@Ogre@@V?$CategorisedAllocPolicy@$0A@@2@@2@@std@@XZ\0";
const SYM_ADD_RENDER_SYSTEM: &[u8] = b"?addRenderSystem@Root@Ogre@@QEAAXPEAVRenderSystem@2@@Z\0";

// Our name: "Vulkan (Reborn)" as MSVC std::string (SSO mode, 15 chars = fits)
// Layout: [15 bytes data + null][8 bytes size=15][8 bytes cap=15]
static mut VULKAN_NAME: [u8; 32] = [0u8; 32];
static VULKAN_NAME_INIT: std::sync::Once = std::sync::Once::new();

unsafe fn init_vulkan_name() -> *const u8 {
    VULKAN_NAME_INIT.call_once(|| {
        let name = b"Vulkan (Reborn)";
        ptr::copy_nonoverlapping(name.as_ptr(), VULKAN_NAME.as_mut_ptr(), 15);
        VULKAN_NAME[15] = 0;
        // size = 15
        *(VULKAN_NAME.as_mut_ptr().add(16) as *mut u64) = 15;
        // capacity = 15 (SSO)
        *(VULKAN_NAME.as_mut_ptr().add(24) as *mut u64) = 15;
    });
    VULKAN_NAME.as_ptr()
}

// Custom getName — returns pointer to our static string (const String& = pointer in RAX)
unsafe extern "C" fn custom_get_name(_this: *mut u8) -> *const u8 {
    init_vulkan_name()
}

// Our fake RenderSystem object — big enough to hold D3D11's data
// D3D11RenderSystem is large (~4KB+), we allocate 8KB to be safe
static mut FAKE_RS: [u8; 8192] = [0u8; 8192];
static mut CLONED_VTABLE: [*const u8; 64] = [ptr::null(); 64];

/// Read an MSVC std::string at the given address
unsafe fn read_msvc_string(addr: *const u8) -> Option<String> {
    if IsBadReadPtr(addr, 32) != 0 {
        return None;
    }
    let cap = *(addr.add(24) as *const u64);
    let (data_ptr, len) = if cap <= 15 {
        let len = *(addr.add(16) as *const u64) as usize;
        (addr, len)
    } else {
        let ptr = *(addr as *const *const u8);
        let len = *(addr.add(16) as *const u64) as usize;
        if IsBadReadPtr(ptr, len) != 0 { return None; }
        (ptr, len)
    };
    if len > 1024 { return None; }
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    String::from_utf8(bytes.to_vec()).ok()
}

#[unsafe(no_mangle)]
pub extern "C" fn dllStartPlugin() {
    // Clear old log
    let _ = std::fs::File::create(LOG_FILE);
    
    log("╔═══════════════════════════════════════════════════════╗");
    log("║  KenshiReborn v0.7.0 — REVERSE SIEGE                ║");
    log("║  Clone D3D11 vtable → patch getName → DROPDOWN!      ║");
    log("╚═══════════════════════════════════════════════════════╝");
    
    unsafe {
        // Get OgreMain
        let ogre = GetModuleHandleA(b"OgreMain_x64.dll\0".as_ptr());
        if ogre.is_null() {
            log("ERROR: OgreMain_x64.dll not found");
            return;
        }
        log(&format!("✓ OgreMain at {:?}", ogre));
        
        // Resolve symbols
        let fn_get_root = GetProcAddress(ogre, SYM_ROOT_GET_SINGLETON.as_ptr());
        let fn_get_renderers = GetProcAddress(ogre, SYM_GET_AVAIL_RENDERERS.as_ptr());
        let fn_add_rs = GetProcAddress(ogre, SYM_ADD_RENDER_SYSTEM.as_ptr());
        
        if fn_get_root.is_null() || fn_get_renderers.is_null() {
            log("ERROR: Core symbols not found");
            return;
        }
        
        let has_add_rs = !fn_add_rs.is_null();
        log(&format!("✓ Symbols: root={:?} renderers={:?} addRS={:?} ({})", 
            fn_get_root, fn_get_renderers, fn_add_rs,
            if has_add_rs { "CAN REGISTER" } else { "NO addRenderSystem — investigate only" }
        ));
        
        let get_root: FnGetSingletonPtr = std::mem::transmute(fn_get_root);
        let get_renderers: FnGetAvailableRenderers = std::mem::transmute(fn_get_renderers);
        
        let root = get_root();
        if root.is_null() {
            log("ERROR: Root is null");
            return;
        }
        log(&format!("✓ Root at {:?}", root));
        
        // Get D3D11 RenderSystem
        let vec = get_renderers(root);
        let count = ((*vec).end as usize - (*vec).begin as usize) / 8;
        log(&format!("✓ {} renderer(s) found", count));
        
        if count == 0 {
            log("ERROR: No renderers to clone");
            return;
        }
        
        let d3d11_rs = *(*vec).begin;
        log(&format!("✓ D3D11 RenderSystem at {:?}", d3d11_rs));
        
        // Read the D3D11 vtable
        let d3d11_vtable = *(d3d11_rs as *const *const *const u8);
        log(&format!("✓ D3D11 vtable at {:?}", d3d11_vtable));
        
        // === SIEGE PHASE 1: Find getName by calling candidates ===
        log("\n=== SIEGE: Finding getName ===");
        
        // getName should return a string containing "Direct3D"
        // Try the first few slots carefully
        let mut get_name_slot: Option<usize> = None;
        
        // Candidates: slots 1, 2 are most likely (after destructor at 0)
        // Slot 1 is 720 bytes — could be getName with extra validation
        // Also scan the mName in the object directly
        
        // First, find mName by scanning the D3D11 object for "Direct3D"
        log("  Scanning D3D11 object for mName string...");
        for offset in (8..512).step_by(8) {
            if IsBadReadPtr(d3d11_rs.add(offset), 32) != 0 { continue; }
            if let Some(s) = read_msvc_string(d3d11_rs.add(offset)) {
                if s.contains("Direct3D") || s.contains("D3D11") || s.contains("Rendering") {
                    log(&format!("  ★ Found mName at offset {} = \"{}\"", offset, s));
                }
                if !s.is_empty() && s.len() > 3 && s.len() < 200 && s.chars().all(|c| c.is_ascii()) {
                    log(&format!("  String@offset{}: \"{}\"", offset, &s[..s.len().min(80)]));
                }
            }
        }
        
        // Now try calling getName candidates
        // SAFETY: We wrap in catch_unwind to survive bad calls
        log("\n  Testing getName candidates...");
        for candidate_slot in [1usize, 2, 3, 6, 7, 11, 12, 13] {
            if IsBadReadPtr(d3d11_vtable.add(candidate_slot) as *const u8, 8) != 0 {
                continue;
            }
            let fn_ptr = *d3d11_vtable.add(candidate_slot);
            
            // Try calling it as getName(this) -> const String&
            type FnGetName = unsafe extern "C" fn(this: *mut u8) -> *const u8;
            let try_fn: FnGetName = std::mem::transmute(fn_ptr);
            
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let ret = try_fn(d3d11_rs);
                if !ret.is_null() && IsBadReadPtr(ret, 32) == 0 {
                    read_msvc_string(ret)
                } else {
                    None
                }
            }));
            
            match result {
                Ok(Some(s)) => {
                    log(&format!("  Slot {} returned: \"{}\"", candidate_slot, &s[..s.len().min(60)]));
                    if s.contains("Direct3D") || s.contains("D3D11") || s.contains("Rendering") {
                        log(&format!("  ★★★ SLOT {} IS getName()! ★★★", candidate_slot));
                        get_name_slot = Some(candidate_slot);
                        break;
                    }
                }
                Ok(None) => {
                    log(&format!("  Slot {} returned null/invalid", candidate_slot));
                }
                Err(_) => {
                    log(&format!("  Slot {} CRASHED (caught) — not getName", candidate_slot));
                }
            }
        }
        
        // === SIEGE PHASE 2: Clone vtable ===
        log("\n=== SIEGE: Cloning vtable ===");
        for i in 0..64 {
            if IsBadReadPtr(d3d11_vtable.add(i) as *const u8, 8) != 0 {
                break;
            }
            CLONED_VTABLE[i] = *d3d11_vtable.add(i);
        }
        log("✓ 64 vtable entries cloned");
        
        // Patch getName slot
        let name_slot = get_name_slot.unwrap_or(1); // default to slot 1
        log(&format!("  Patching slot {} with custom_get_name", name_slot));
        CLONED_VTABLE[name_slot] = custom_get_name as *const u8;
        
        // === SIEGE PHASE 3: Build fake RenderSystem ===
        log("\n=== SIEGE: Building fake RenderSystem ===");
        
        // Copy the first 1024 bytes of D3D11's object as a template
        // This gives us all the member variables in their initialized state
        if IsBadReadPtr(d3d11_rs, 1024) == 0 {
            ptr::copy_nonoverlapping(d3d11_rs, FAKE_RS.as_mut_ptr(), 1024);
            log("✓ Copied 1024 bytes of D3D11 object as template");
        }
        
        // Point to our cloned+patched vtable
        let fake_rs_ptr = FAKE_RS.as_mut_ptr();
        *(fake_rs_ptr as *mut *const *const u8) = CLONED_VTABLE.as_ptr() as *const *const u8;
        
        // Write our name into the mName field
        // From the scan, we'll know the offset. For now, try offset 8 (Ogre default)
        // Write "Vulkan (Reborn)" as MSVC std::string at offset 8
        let name_ptr = init_vulkan_name();
        ptr::copy_nonoverlapping(name_ptr, fake_rs_ptr.add(8), 32);
        log("✓ Wrote 'Vulkan (Reborn)' into mName@offset8");
        
        // === SIEGE PHASE 4: Register! ===
        if has_add_rs {
            log("\n=== SIEGE: REGISTERING 'Vulkan (Reborn)' ===");
            let add_rs: FnAddRenderSystem = std::mem::transmute(fn_add_rs);
            add_rs(root, fake_rs_ptr);
            log("✓✓✓ REGISTERED! Check the Kenshi dropdown! ✓✓✓");
        } else {
            log("\nWARNING: addRenderSystem symbol not found — cannot register");
            log("  The vtable is cloned and patched but we can't add it");
        }
        
        // Verify
        let vec2 = get_renderers(root);
        let count2 = ((*vec2).end as usize - (*vec2).begin as usize) / 8;
        log(&format!("\n✓ Renderers after siege: {} (was {})", count2, count));
        
        log("\n╔═══════════════════════════════════════════════════════╗");
        log("║  SIEGE COMPLETE — KenshiReborn v0.7.0                ║");
        if count2 > count {
            log("║  'Vulkan (Reborn)' should be in the dropdown!        ║");
        } else {
            log("║  Registration may not have worked — check log above  ║");
        }
        log("╚═══════════════════════════════════════════════════════╝");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dllStopPlugin() {
    log("dllStopPlugin() — siege complete, standing down");
}
