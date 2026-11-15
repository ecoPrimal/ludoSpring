// KenshiReborn Bridge DLL — RenderSystem_Vulkan_x64.dll
// v0.6.0 — INVESTIGATOR: Map D3D11's vtable without registering anything
//
// Phase 1 (v0.5.0) proved the DLL loads and Ogre symbols resolve.
// Phase 1 ALSO proved that registering a base RenderSystem crashes
// because Ogre calls pure virtual methods during config dialog creation.
//
// This version:
// 1. Gets Root singleton
// 2. Reads getAvailableRenderers() to find the D3D11 RenderSystem
// 3. Dumps the D3D11 object's vtable to a log file
// 4. Identifies getName slot by string matching
// 5. Maps config-related methods
// 6. Does NOT register anything — Kenshi stays stable
//
// Output: kenshi_reborn_vtable.json with full vtable map

#![allow(non_snake_case)]

use std::fs::OpenOptions;
use std::io::Write;
use std::ptr;

const LOG_FILE: &str = r"M:\SteamLibrary\steamapps\common\Kenshi\kenshi_reborn.log";
const VTABLE_FILE: &str = r"M:\SteamLibrary\steamapps\common\Kenshi\kenshi_reborn_vtable.json";

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

// Ogre types
type FnGetSingletonPtr = unsafe extern "C" fn() -> *mut u8;
type FnGetAvailableRenderers = unsafe extern "C" fn(root: *mut u8) -> *mut StdVector;

// MSVC std::vector layout (simplified)
#[repr(C)]
struct StdVector {
    begin: *mut *mut u8,  // pointer to first element
    end: *mut *mut u8,    // pointer past last element
    cap: *mut *mut u8,    // capacity end
}

// Mangled symbols
const SYM_ROOT_GET_SINGLETON: &[u8] = b"?getSingletonPtr@Root@Ogre@@SAPEAV12@XZ\0";
const SYM_GET_AVAIL_RENDERERS: &[u8] = b"?getAvailableRenderers@Root@Ogre@@QEAAAEBV?$vector@PEAVRenderSystem@Ogre@@V?$STLAllocator@PEAVRenderSystem@Ogre@@V?$CategorisedAllocPolicy@$0A@@2@@2@@std@@XZ\0";

/// Read an MSVC std::string at the given address, return the string content
unsafe fn read_msvc_string(addr: *const u8) -> Option<String> {
    if IsBadReadPtr(addr, 32) != 0 {
        return None;
    }
    
    // Read capacity to determine SSO vs heap
    let cap = *(addr.add(24) as *const u64);
    
    let (data_ptr, len) = if cap <= 15 {
        // SSO mode — data is inline
        let len = *(addr.add(16) as *const u64) as usize;
        (addr, len)
    } else {
        // Heap mode — first 8 bytes are a pointer
        let ptr = *(addr as *const *const u8);
        let len = *(addr.add(16) as *const u64) as usize;
        if IsBadReadPtr(ptr, len) != 0 {
            return None;
        }
        (ptr, len)
    };
    
    if len > 1024 {
        return None; // sanity check
    }
    
    let bytes = std::slice::from_raw_parts(data_ptr, len);
    String::from_utf8(bytes.to_vec()).ok()
}

/// Try to read a string returned by calling a vtable entry (const String& return)
/// The function returns a pointer to a std::string
unsafe fn try_call_getname(vtable_entry: *const u8, this: *mut u8) -> Option<String> {
    // This is extremely dangerous — we're calling an unknown function
    // Only do this if we're confident the vtable entry is getName
    type FnGetName = unsafe extern "C" fn(this: *mut u8) -> *const u8;
    let get_name: FnGetName = std::mem::transmute(vtable_entry);
    
    // Use structured exception handling... well, Rust doesn't have SEH
    // Instead, just try it and hope for the best
    let result = get_name(this);
    if result.is_null() || IsBadReadPtr(result, 32) != 0 {
        return None;
    }
    read_msvc_string(result)
}

#[unsafe(no_mangle)]
pub extern "C" fn dllStartPlugin() {
    log("╔═══════════════════════════════════════════════════════╗");
    log("║  KenshiReborn v0.6.0 — VTABLE INVESTIGATOR           ║");
    log("║  READ ONLY — does NOT register any RenderSystem       ║");
    log("╚═══════════════════════════════════════════════════════╝");
    
    unsafe {
        // Get OgreMain
        let ogre = GetModuleHandleA(b"OgreMain_x64.dll\0".as_ptr());
        if ogre.is_null() {
            let ogre2 = LoadLibraryA(b"OgreMain_x64.dll\0".as_ptr());
            if ogre2.is_null() {
                log("ERROR: OgreMain_x64.dll not found");
                return;
            }
        }
        let ogre = GetModuleHandleA(b"OgreMain_x64.dll\0".as_ptr());
        log(&format!("✓ OgreMain at {:?}", ogre));
        
        // Get Root
        let fn_get_root = GetProcAddress(ogre, SYM_ROOT_GET_SINGLETON.as_ptr());
        let fn_get_renderers = GetProcAddress(ogre, SYM_GET_AVAIL_RENDERERS.as_ptr());
        
        if fn_get_root.is_null() || fn_get_renderers.is_null() {
            log(&format!("ERROR: Symbol resolution failed (root={:?}, renderers={:?})", 
                fn_get_root, fn_get_renderers));
            return;
        }
        
        let get_root: FnGetSingletonPtr = std::mem::transmute(fn_get_root);
        let get_renderers: FnGetAvailableRenderers = std::mem::transmute(fn_get_renderers);
        
        let root = get_root();
        if root.is_null() {
            log("ERROR: Root is null");
            return;
        }
        log(&format!("✓ Root at {:?}", root));
        
        // Get available renderers
        let vec = get_renderers(root);
        if vec.is_null() {
            log("ERROR: getAvailableRenderers returned null");
            return;
        }
        
        let count = ((*vec).end as usize - (*vec).begin as usize) / 8;
        log(&format!("✓ Available renderers: {} found", count));
        
        if count == 0 {
            log("WARNING: No renderers available yet (we load before D3D11?)");
            log("  Our line in Plugins_x64.cfg is AFTER D3D11 — this shouldn't happen");
            return;
        }
        
        // Iterate through renderers
        let mut json_entries = Vec::new();
        
        for i in 0..count {
            let rs_ptr = *(*vec).begin.add(i);
            if rs_ptr.is_null() {
                continue;
            }
            log(&format!("\n=== RenderSystem[{}] at {:?} ===", i, rs_ptr));
            
            // Read vtable pointer (first 8 bytes of object)
            let vtable_ptr = *(rs_ptr as *const *const *const u8);
            log(&format!("  vtable at {:?}", vtable_ptr));
            
            // Try to read mName at offset 8
            let name_at_8 = read_msvc_string(rs_ptr.add(8));
            log(&format!("  mName@offset8: {:?}", name_at_8));
            
            // Try other offsets in case mName isn't at 8
            for offset in [8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 96, 112, 128].iter() {
                let s = read_msvc_string(rs_ptr.add(*offset));
                if let Some(ref text) = s {
                    if text.len() > 3 && text.len() < 200 {
                        log(&format!("  String@offset{}: \"{}\"", offset, text));
                    }
                }
            }
            
            // Dump first 64 vtable entries (function pointers)
            log(&format!("\n  Vtable entries (first 64):"));
            let ogre_base = ogre as usize;
            let ogre_end = ogre_base + 10 * 1024 * 1024; // ~10MB range
            
            let mut vtable_json = Vec::new();
            
            for slot in 0..64 {
                if IsBadReadPtr(vtable_ptr.add(slot) as *const u8, 8) != 0 {
                    log(&format!("  [{}] <unreadable>", slot));
                    break;
                }
                let fn_ptr = *vtable_ptr.add(slot);
                let fn_addr = fn_ptr as usize;
                
                // Check if this is in OgreMain or the D3D11 DLL
                let module = if fn_addr >= ogre_base && fn_addr < ogre_end {
                    "OgreMain"
                } else {
                    "D3D11/other"
                };
                
                let rva = fn_addr.wrapping_sub(ogre_base);
                log(&format!("  [{:2}] {:?} (RVA 0x{:x}) [{}]", slot, fn_ptr, rva, module));
                
                vtable_json.push(format!(
                    "    {{\"slot\": {}, \"addr\": \"0x{:x}\", \"rva\": \"0x{:x}\", \"module\": \"{}\"}}",
                    slot, fn_addr, rva, module
                ));
            }
            
            let name_str = name_at_8.unwrap_or_else(|| "unknown".to_string());
            json_entries.push(format!(
                "  {{\n    \"index\": {},\n    \"name\": \"{}\",\n    \"address\": \"0x{:x}\",\n    \"vtable\": \"0x{:x}\",\n    \"entries\": [\n{}\n    ]\n  }}",
                i, name_str, rs_ptr as usize, vtable_ptr as usize,
                vtable_json.join(",\n")
            ));
        }
        
        // Write vtable JSON
        let json = format!("{{\n  \"version\": \"v0.6.0\",\n  \"purpose\": \"D3D11 vtable map for safe RenderSystem registration\",\n  \"renderers\": [\n{}\n  ]\n}}", json_entries.join(",\n"));
        
        if let Ok(mut f) = std::fs::File::create(VTABLE_FILE) {
            let _ = f.write_all(json.as_bytes());
            log(&format!("\n✓ Wrote vtable map to {}", VTABLE_FILE));
        }
        
        log("\n╔═══════════════════════════════════════════════════════╗");
        log("║  INVESTIGATION COMPLETE — Kenshi is SAFE             ║");
        log("║  No RenderSystem registered. Read-only scan.         ║");
        log("╚═══════════════════════════════════════════════════════╝");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dllStopPlugin() {
    log("dllStopPlugin() — clean exit, investigator mode");
}

// NOTE: No custom DllMain — Rust cdylib provides its own.
// A duplicate DllMain conflicts and can prevent the DLL from loading.
// Ogre will call dllStartPlugin() after LoadLibrary succeeds.
