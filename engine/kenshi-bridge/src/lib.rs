// KenshiReborn Bridge DLL — RenderSystem_Vulkan_x64.dll
// v0.7.7 — NAME HUNT: Find mName in the RS object, overwrite it
//
// v0.7.6 proved: clone D3D11 = no crash, launcher shows 2 entries!
// But both say "Direct3D 11" because getName() returns a static ref
// and the launcher reads the name from the object's mName member.
// We need to FIND that member and OVERWRITE it.

#![allow(non_snake_case, unused)]

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
    fn GetProcAddress(module: *mut u8, name: *const u8) -> *mut u8;
    fn GetModuleHandleA(name: *const u8) -> *mut u8;
    fn VirtualAlloc(addr: *mut u8, size: usize, alloc_type: u32, protect: u32) -> *mut u8;
}

const MEM_COMMIT: u32 = 0x1000;
const MEM_RESERVE: u32 = 0x2000;
const PAGE_READWRITE: u32 = 0x04;

type FnGetSingletonPtr = unsafe extern "C" fn() -> *mut u8;
type FnGetAvailableRenderers = unsafe extern "C" fn(root: *mut u8) -> *mut StdVector;
type FnAddRenderSystem = unsafe extern "C" fn(root: *mut u8, rs: *mut u8);

#[repr(C)]
struct StdVector {
    begin: *mut *mut u8,
    end: *mut *mut u8,
    cap: *mut *mut u8,
}

const SYM_ROOT_GET_SINGLETON: &[u8] = b"?getSingletonPtr@Root@Ogre@@SAPEAV12@XZ\0";
const SYM_GET_AVAIL_RENDERERS: &[u8] = b"?getAvailableRenderers@Root@Ogre@@QEAAAEBV?$vector@PEAVRenderSystem@Ogre@@V?$STLAllocator@PEAVRenderSystem@Ogre@@V?$CategorisedAllocPolicy@$0A@@2@@2@@std@@XZ\0";
const SYM_ADD_RENDER_SYSTEM: &[u8] = b"?addRenderSystem@Root@Ogre@@QEAAXPEAVRenderSystem@2@@Z\0";

// Our name as a heap-style MSVC string (matching D3D11's format)
static mut OUR_NAME_BUF: [u8; 64] = [0u8; 64];  // "Vulkan (Reborn)\0" buffer
static mut OUR_NAME_STR: [u8; 32] = [0u8; 32];   // MSVC string object pointing to buf

unsafe extern "C" fn custom_get_name(_this: *mut u8) -> *const u8 {
    OUR_NAME_STR.as_ptr()
}

static mut CLONE_VTABLE: [u64; 128] = [0u64; 128];

#[unsafe(no_mangle)]
pub extern "C" fn dllStartPlugin() {
    let _ = std::fs::File::create(LOG_FILE);
    
    log("╔═══════════════════════════════════════════════════════╗");
    log("║  KenshiReborn v0.7.7 — NAME HUNT                    ║");
    log("║  Find mName in the RS object, write Vulkan (Reborn)  ║");
    log("╚═══════════════════════════════════════════════════════╝");
    
    unsafe {
        let ogre = GetModuleHandleA(b"OgreMain_x64.dll\0".as_ptr());
        if ogre.is_null() { log("ERROR: OgreMain not found"); return; }
        
        let fn_get_root = GetProcAddress(ogre, SYM_ROOT_GET_SINGLETON.as_ptr());
        let fn_get_renderers = GetProcAddress(ogre, SYM_GET_AVAIL_RENDERERS.as_ptr());
        let fn_add_rs = GetProcAddress(ogre, SYM_ADD_RENDER_SYSTEM.as_ptr());
        
        if fn_get_root.is_null() || fn_get_renderers.is_null() || fn_add_rs.is_null() {
            log("ERROR: Symbol resolution failed"); return;
        }
        
        let get_root: FnGetSingletonPtr = std::mem::transmute(fn_get_root);
        let get_renderers: FnGetAvailableRenderers = std::mem::transmute(fn_get_renderers);
        let add_rs: FnAddRenderSystem = std::mem::transmute(fn_add_rs);
        
        let root = get_root();
        if root.is_null() { log("ERROR: Root is null"); return; }
        
        let vec = get_renderers(root);
        let count = ((*vec).end as usize - (*vec).begin as usize) / 8;
        if count == 0 { log("ERROR: No renderers"); return; }
        
        let d3d11_rs = *(*vec).begin;
        log(&format!("  D3D11 RS at 0x{:x}", d3d11_rs as u64));
        
        // ═══ GET THE D3D11 NAME ═══
        let d3d11_vtable = *(d3d11_rs as *const u64) as *const u64;
        let d3d11_get_name: unsafe extern "C" fn(*mut u8) -> *const u8 = 
            std::mem::transmute(*d3d11_vtable.add(1));
        let name_str_ptr = d3d11_get_name(d3d11_rs);
        
        // Read the string: first 8 bytes = heap pointer (if size > 15)
        let heap_ptr = *(name_str_ptr as *const u64);
        let str_size = *(name_str_ptr.add(16) as *const u64);
        let str_cap = *(name_str_ptr.add(24) as *const u64);
        
        log(&format!("  D3D11 name: str_obj=0x{:x}, heap=0x{:x}, size={}, cap={}", 
            name_str_ptr as u64, heap_ptr, str_size, str_cap));
        
        // Read the actual chars
        let char_ptr = if str_size > 15 { heap_ptr as *const u8 } else { name_str_ptr };
        let mut name_bytes = Vec::new();
        for i in 0..str_size.min(100) as usize {
            let b = *char_ptr.add(i);
            if b == 0 { break; }
            name_bytes.push(b);
        }
        log(&format!("  D3D11 name text: '{}'", String::from_utf8_lossy(&name_bytes)));
        
        // ═══ SCAN the RS object for the name string pointer ═══
        // Search for the string object pointer OR the heap data pointer
        let scan_size: usize = 4096;
        let target_addr = name_str_ptr as u64;
        let target_heap = heap_ptr;
        
        log(&format!("  Scanning {} bytes for name pointers...", scan_size));
        
        let mut found_offsets = Vec::new();
        for offset in (0..scan_size).step_by(8) {
            let val = *(d3d11_rs.add(offset) as *const u64);
            if val == target_addr {
                log(&format!("    FOUND str_obj ptr at offset {} (0x{:x})", offset, offset));
                found_offsets.push((offset, "str_obj"));
            }
            if val == target_heap && target_heap != 0 {
                log(&format!("    FOUND heap ptr at offset {} (0x{:x})", offset, offset));
                found_offsets.push((offset, "heap"));
            }
        }
        
        // Also scan for embedded SSO-style pattern:
        // Look for "Dire" (0x65726944) in the object  
        let dire_pattern: u32 = 0x65726944; // "Dire" in little-endian
        for offset in 0..scan_size-4 {
            let val = *(d3d11_rs.add(offset) as *const u32);
            if val == dire_pattern {
                log(&format!("    FOUND 'Dire' text at offset {} (0x{:x})", offset, offset));
                found_offsets.push((offset, "text"));
            }
        }
        
        if found_offsets.is_empty() {
            log("  WARNING: Name not found in first 4KB — scanning wider...");
            for offset in (4096..8192).step_by(8) {
                let val = *(d3d11_rs.add(offset) as *const u64);
                if val == target_addr || (val == target_heap && target_heap != 0) {
                    log(&format!("    FOUND ptr at offset {} (0x{:x})", offset, offset));
                    found_offsets.push((offset, "ptr"));
                }
            }
        }
        
        // ═══ CLONE ═══
        let clone_size: usize = 8192;
        let clone = VirtualAlloc(ptr::null_mut(), clone_size, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
        if clone.is_null() { log("ERROR: VirtualAlloc failed"); return; }
        
        ptr::copy_nonoverlapping(d3d11_rs, clone, clone_size);
        log(&format!("  Clone at 0x{:x}", clone as u64));
        
        // ═══ SET UP OUR NAME ═══
        // Use heap-style string matching D3D11's format
        let our_text = b"Vulkan (Reborn)\0";
        ptr::copy_nonoverlapping(our_text.as_ptr(), OUR_NAME_BUF.as_mut_ptr(), our_text.len());
        
        // Build the string object: heap ptr, size, capacity
        let name_obj = OUR_NAME_STR.as_mut_ptr();
        *(name_obj as *mut u64) = OUR_NAME_BUF.as_ptr() as u64;  // heap pointer
        *(name_obj.add(16) as *mut u64) = 15;                     // size
        *(name_obj.add(24) as *mut u64) = 63;                     // capacity
        
        log(&format!("  Our name str_obj at 0x{:x}, buf at 0x{:x}", 
            OUR_NAME_STR.as_ptr() as u64, OUR_NAME_BUF.as_ptr() as u64));
        
        // ═══ OVERWRITE NAME in clone ═══
        for (offset, kind) in &found_offsets {
            match *kind {
                "str_obj" => {
                    // Replace string object pointer with ours
                    *(clone.add(*offset) as *mut u64) = OUR_NAME_STR.as_ptr() as u64;
                    log(&format!("  Overwrote str_obj ptr at clone+{}", offset));
                },
                "heap" => {
                    // Replace heap data pointer with our buffer
                    *(clone.add(*offset) as *mut u64) = OUR_NAME_BUF.as_ptr() as u64;
                    log(&format!("  Overwrote heap ptr at clone+{}", offset));
                },
                "text" => {
                    // Overwrite inline text
                    let dest = clone.add(*offset);
                    ptr::copy_nonoverlapping(our_text.as_ptr(), dest, our_text.len());
                    // Zero remaining bytes to be safe
                    ptr::write_bytes(dest.add(our_text.len()), 0, 16);
                    log(&format!("  Overwrote inline text at clone+{}", offset));
                },
                _ => {
                    *(clone.add(*offset) as *mut u64) = OUR_NAME_STR.as_ptr() as u64;
                    log(&format!("  Overwrote ptr at clone+{}", offset));
                }
            }
        }
        
        // ═══ CLONE VTABLE, swap getName ═══
        ptr::copy_nonoverlapping(d3d11_vtable as *const u8, CLONE_VTABLE.as_mut_ptr() as *mut u8, 128 * 8);
        CLONE_VTABLE[1] = custom_get_name as u64;
        *(clone as *mut u64) = CLONE_VTABLE.as_ptr() as u64;
        
        // ═══ REGISTER ═══
        add_rs(root, clone);
        
        let vec2 = get_renderers(root);
        let count2 = ((*vec2).end as usize - (*vec2).begin as usize) / 8;
        log(&format!("  Renderers: {} → {}", count, count2));
        
        // Verify by calling getName on our clone through the vtable
        let our_vtable = *(clone as *const u64) as *const u64;
        let our_get_name: unsafe extern "C" fn(*mut u8) -> *const u8 = 
            std::mem::transmute(*our_vtable.add(1));
        let verify_ptr = our_get_name(clone);
        let vhp = *(verify_ptr as *const u64);
        let vsz = *(verify_ptr.add(16) as *const u64);
        let vcp = if vsz > 15 { vhp as *const u8 } else { verify_ptr };
        let mut vname = Vec::new();
        for i in 0..vsz.min(50) as usize {
            let b = *vcp.add(i);
            if b == 0 { break; }
            vname.push(b);
        }
        
        log("");
        log("╔═══════════════════════════════════════════════════════╗");
        log(&format!("║  getName() → '{}'", String::from_utf8_lossy(&vname)));
        log(&format!("║  Name offsets found: {}", found_offsets.len()));
        log("║  The dough rises — the old was eaten and renamed!    ║");
        log("╚═══════════════════════════════════════════════════════╝");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dllStopPlugin() {
    log("dllStopPlugin()");
}
