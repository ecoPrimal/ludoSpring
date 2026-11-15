// KenshiReborn Bridge DLL — RenderSystem_Vulkan_x64.dll
// v0.7.8 — THE NAME LIVES: Proper MSVC SSO string for getName()
//
// v0.7.6: Clone works! Kenshi launches! Two entries in dropdown!
// v0.7.7: Name hunt — mName NOT in RS object (0 offsets found)
//         The name is a STATIC string in the D3D11 DLL
//         Launcher calls getName() through vtable → we CAN rename!
//         Bug: our string was heap-format but size=15 → MSVC reads SSO → empty
// v0.7.8: Fix: proper SSO format for "Vulkan (Reborn)" (exactly 15 chars!)
//         The scaffold pattern: eat old, clone, rename, bridge across all

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

// ═══════════════════════════════════════════════════════════
// THE NAME — "Vulkan (Reborn)" as proper MSVC std::string
// ═══════════════════════════════════════════════════════════
// "Vulkan (Reborn)" = 15 chars = EXACTLY fits SSO buffer!
// MSVC SSO layout (32 bytes):
//   [0-15]  = inline char data (up to 15 chars + null)
//   [16-23] = _Mysize (u64)
//   [24-31] = _Myres  (u64) — if < 16, string is SSO

// IMPORTANT: This must be static with stable address since we return &String
static mut VULKAN_NAME: [u8; 32] = [0u8; 32];
static NAME_INIT: std::sync::Once = std::sync::Once::new();

unsafe fn get_vulkan_name() -> *const u8 {
    NAME_INIT.call_once(|| {
        let n = VULKAN_NAME.as_mut_ptr();
        // SSO inline data: "Vulkan (Reborn)" + null
        let text = b"Vulkan (Reborn)\0";
        ptr::copy_nonoverlapping(text.as_ptr(), n, 16);
        // _Mysize = 15
        *(n.add(16) as *mut u64) = 15;
        // _Myres = 15 (indicates SSO — value < 16)
        *(n.add(24) as *mut u64) = 15;
    });
    VULKAN_NAME.as_ptr()
}

// getName() → const String& — returns reference to our SSO string
unsafe extern "C" fn custom_get_name(_this: *mut u8) -> *const u8 {
    get_vulkan_name()
}

static mut CLONE_VTABLE: [u64; 128] = [0u64; 128];

#[unsafe(no_mangle)]
pub extern "C" fn dllStartPlugin() {
    let _ = std::fs::File::create(LOG_FILE);
    
    log("╔═══════════════════════════════════════════════════════╗");
    log("║  KenshiReborn v0.7.8 — THE NAME LIVES               ║");
    log("║  Proper SSO string: 'Vulkan (Reborn)' in dropdown!   ║");
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
        let d3d11_vtable = *(d3d11_rs as *const u64) as *const u64;
        
        log(&format!("  D3D11 RS at 0x{:x}", d3d11_rs as u64));
        
        // Initialize our name
        let our_name = get_vulkan_name();
        
        // Verify SSO reads correctly
        let verify_sz = *(our_name.add(16) as *const u64);
        let verify_res = *(our_name.add(24) as *const u64);
        let is_sso = verify_res < 16;
        let char_ptr = if is_sso { our_name } else { *(our_name as *const u64) as *const u8 };
        let mut verify_str = Vec::new();
        for i in 0..verify_sz as usize {
            let b = *char_ptr.add(i);
            if b == 0 { break; }
            verify_str.push(b);
        }
        log(&format!("  Our name: '{}' (size={}, res={}, sso={})", 
            String::from_utf8_lossy(&verify_str), verify_sz, verify_res, is_sso));
        
        // ═══ CLONE D3D11 ═══
        let clone_size: usize = 8192;
        let clone = VirtualAlloc(ptr::null_mut(), clone_size, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
        if clone.is_null() { log("ERROR: VirtualAlloc failed"); return; }
        
        ptr::copy_nonoverlapping(d3d11_rs, clone, clone_size);
        
        // ═══ CLONE VTABLE, swap getName ═══
        ptr::copy_nonoverlapping(d3d11_vtable as *const u8, CLONE_VTABLE.as_mut_ptr() as *mut u8, 128 * 8);
        CLONE_VTABLE[1] = custom_get_name as u64;
        *(clone as *mut u64) = CLONE_VTABLE.as_ptr() as u64;
        
        log("  Cloned D3D11 + vtable, swapped getName");
        
        // ═══ REGISTER ═══
        add_rs(root, clone);
        
        let vec2 = get_renderers(root);
        let count2 = ((*vec2).end as usize - (*vec2).begin as usize) / 8;
        log(&format!("  Renderers: {} → {}", count, count2));
        
        log("");
        log("╔═══════════════════════════════════════════════════════╗");
        log("║  ★ 'Vulkan (Reborn)' — THE NAME LIVES ★            ║");
        log("║  D3D11 scaffold + SSO getName = dropdown entry!      ║");
        log("║  The scaffold pattern: eat → clone → rename → bridge ║");
        log("╚═══════════════════════════════════════════════════════╝");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dllStopPlugin() {
    log("dllStopPlugin() — name stands, scaffold holds");
}
