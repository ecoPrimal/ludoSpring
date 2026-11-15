// KenshiReborn Bridge DLL — RenderSystem_Vulkan_x64.dll
// v0.7.6 — EAT THE OLD: Clone D3D11 as scaffold, rename to Vulkan
//
// sourDough log:
//   v0.7.0-v0.7.4: hand-built fake RS → always crashes after slot 3
//   v0.7.5: no-op diagnostic (pending test)
//   v0.7.6: EAT THE OLD — clone the WORKING D3D11 RenderSystem object
//           Copy its ENTIRE memory layout (vtable, members, pointers)
//           Replace ONLY the name: "Vulkan (Reborn)"
//           The old D3D11 becomes the scaffold. The dough rises on bones.

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

fn log_hex(label: &str, ptr: *const u8, len: usize) {
    unsafe {
        let mut hex = String::new();
        for i in 0..len.min(64) {
            hex.push_str(&format!("{:02x} ", *ptr.add(i)));
            if (i + 1) % 16 == 0 { hex.push_str("\n    "); }
        }
        log(&format!("  {} @ 0x{:x}:\n    {}", label, ptr as u64, hex));
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

// Custom getName that returns OUR name
static mut OUR_NAME: [u8; 32] = [0u8; 32];

unsafe extern "C" fn custom_get_name(_this: *mut u8) -> *const u8 {
    OUR_NAME.as_ptr()
}

// Clone vtable with our getName swapped in
static mut CLONE_VTABLE: [u64; 128] = [0u64; 128];

#[unsafe(no_mangle)]
pub extern "C" fn dllStartPlugin() {
    let _ = std::fs::File::create(LOG_FILE);
    
    log("╔═══════════════════════════════════════════════════════╗");
    log("║  KenshiReborn v0.7.6 — EAT THE OLD                  ║");
    log("║  Clone D3D11 scaffold, rename to Vulkan (Reborn)     ║");
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
        log(&format!("  {} existing renderer(s)", count));
        
        if count == 0 {
            log("ERROR: No existing renderers to clone!"); return;
        }
        
        // ═══ EAT THE OLD: Get the D3D11 RenderSystem pointer ═══
        let d3d11_rs = *(*vec).begin;
        let d3d11_vtable_ptr = *(d3d11_rs as *const u64) as *const u64;
        
        log(&format!("  D3D11 RS at 0x{:x}", d3d11_rs as u64));
        log(&format!("  D3D11 vtable at 0x{:x}", d3d11_vtable_ptr as u64));
        
        // Dump D3D11 object header for study
        log_hex("D3D11 RS object", d3d11_rs, 64);
        
        // Read D3D11's getName to find the name location
        // Slot 1 = getName: call it to see what it returns
        let d3d11_get_name: unsafe extern "C" fn(*mut u8) -> *const u8 = 
            std::mem::transmute(*d3d11_vtable_ptr.add(1));
        let d3d11_name_ptr = d3d11_get_name(d3d11_rs);
        log(&format!("  D3D11 getName returned 0x{:x}", d3d11_name_ptr as u64));
        log_hex("D3D11 name string", d3d11_name_ptr, 48);
        
        // The name ptr should be inside the RS object (mName member)
        let name_offset = d3d11_name_ptr as i64 - d3d11_rs as i64;
        log(&format!("  Name at offset {} from RS base", name_offset));
        
        // ═══ MEASURE the D3D11 RS object size ═══
        // We'll clone a generous amount — 8KB should cover everything
        let clone_size: usize = 8192;
        
        // ═══ ALLOCATE: proper heap memory for our clone ═══
        let clone = VirtualAlloc(
            ptr::null_mut(), 
            clone_size,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE
        );
        
        if clone.is_null() {
            log("ERROR: VirtualAlloc failed"); return;
        }
        
        log(&format!("  Clone allocated at 0x{:x} ({} bytes)", clone as u64, clone_size));
        
        // ═══ CLONE: byte-for-byte copy of D3D11 RS ═══
        ptr::copy_nonoverlapping(d3d11_rs, clone, clone_size);
        log("  Cloned D3D11 RS (8KB byte-for-byte copy)");
        
        // ═══ CLONE VTABLE: copy D3D11's vtable, swap getName ═══
        ptr::copy_nonoverlapping(
            d3d11_vtable_ptr as *const u8,
            CLONE_VTABLE.as_mut_ptr() as *mut u8,
            128 * 8  // 128 slots × 8 bytes
        );
        
        // Swap slot 1 (getName) with our custom version
        CLONE_VTABLE[1] = custom_get_name as u64;
        
        log("  Cloned vtable (128 slots), swapped getName at slot 1");
        
        // ═══ WRITE: Set up our clone ═══
        // Point clone's vtable to our cloned vtable
        *(clone as *mut u64) = CLONE_VTABLE.as_ptr() as u64;
        
        // Write "Vulkan (Reborn)" as SSO string
        let name = b"Vulkan (Reborn)";
        let name_buf = OUR_NAME.as_mut_ptr();
        ptr::copy_nonoverlapping(name.as_ptr(), name_buf, name.len());
        *name_buf.add(name.len()) = 0;
        *(name_buf.add(16) as *mut u64) = name.len() as u64;
        *(name_buf.add(24) as *mut u64) = 15; // SSO capacity
        
        // Also write the name into the clone object at the same offset
        if name_offset > 0 && name_offset < clone_size as i64 - 32 {
            let clone_name = clone.add(name_offset as usize);
            ptr::copy_nonoverlapping(name.as_ptr(), clone_name, name.len());
            *clone_name.add(name.len()) = 0;
            *(clone_name.add(16) as *mut u64) = name.len() as u64;
            *(clone_name.add(24) as *mut u64) = 15;
            log(&format!("  Wrote 'Vulkan (Reborn)' at clone+{}", name_offset));
        }
        
        log_hex("Clone object header", clone, 64);
        
        // ═══ REGISTER the clone ═══
        add_rs(root, clone);
        
        let vec2 = get_renderers(root);
        let count2 = ((*vec2).end as usize - (*vec2).begin as usize) / 8;
        log(&format!("  Renderers: {} → {}", count, count2));
        
        // Verify our clone's name
        let verify_name: unsafe extern "C" fn(*mut u8) -> *const u8 = 
            std::mem::transmute(CLONE_VTABLE[1]);
        let our_name = verify_name(clone);
        
        // Read the string
        let mut name_str = Vec::new();
        let mut i = 0;
        while *our_name.add(i) != 0 && i < 100 {
            name_str.push(*our_name.add(i));
            i += 1;
        }
        let name_display = String::from_utf8_lossy(&name_str);
        
        log("");
        log("╔═══════════════════════════════════════════════════════╗");
        log(&format!("║  ★ THE OLD IS EATEN — '{}' LIVES ★", name_display));
        log("║  D3D11 scaffold: all internals intact                ║");
        log("║  Only the name changed. The dough rises on bones.    ║");
        log("╚═══════════════════════════════════════════════════════╝");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dllStopPlugin() {
    log("dllStopPlugin() — the old was honored, the new lives");
}
