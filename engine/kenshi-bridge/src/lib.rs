// KenshiReborn Bridge DLL — RenderSystem_Vulkan_x64.dll  
// v0.8.0 — THE RENAME: vtable[2] is the SHORT name shown in dropdown!
//
// DISCOVERY (v0.7.9 diagnostic):
//   vtable[1] = getName() -> "Direct3D11 Rendering Subsystem" (30 chars, heap)
//   vtable[2] = ??? -> "Direct3D 11" (short name used by launcher dropdown!)
//   Both return static strings OUTSIDE the RS object
//   Swapping vtable[1] didn't change dropdown because launcher reads vtable[2]!
//
// v0.8.0: Swap BOTH vtable[1] AND vtable[2] to return our name

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
    fn IsBadReadPtr(lp: *const u8, ucb: usize) -> i32;
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
// TWO NAMES — Long name (vtable[1]) and Short name (vtable[2])
// ═══════════════════════════════════════════════════════════
// Long:  "Vulkan (Reborn)" = 15 chars (SSO)
// Short: "Vulkan Reborn"   = 13 chars (SSO)
// Both fit MSVC SSO (<=15 chars, inline, no heap)

static mut LONG_NAME: [u8; 32] = [0u8; 32];
static mut SHORT_NAME: [u8; 32] = [0u8; 32];
static NAME_INIT: std::sync::Once = std::sync::Once::new();

unsafe fn init_names() {
    NAME_INIT.call_once(|| {
        // Long name: "Vulkan (Reborn)" — 15 chars, returned by getName() [vtable[1]]
        let n = LONG_NAME.as_mut_ptr();
        let text = b"Vulkan (Reborn)\0";
        ptr::copy_nonoverlapping(text.as_ptr(), n, 16);
        *(n.add(16) as *mut u64) = 15;  // _Mysize
        *(n.add(24) as *mut u64) = 15;  // _Myres (< 16 = SSO)
        
        // Short name: "Vulkan Reborn" — 13 chars, for dropdown [vtable[2]]
        let s = SHORT_NAME.as_mut_ptr();
        let text2 = b"Vulkan Reborn\0\0\0";
        ptr::copy_nonoverlapping(text2.as_ptr(), s, 16);
        *(s.add(16) as *mut u64) = 13;  // _Mysize
        *(s.add(24) as *mut u64) = 15;  // _Myres (< 16 = SSO)
    });
}

// vtable[1]: getName() -> const String& (long name)
unsafe extern "C" fn custom_get_name(_this: *mut u8) -> *const u8 {
    LONG_NAME.as_ptr()
}

// vtable[2]: getShortName or similar -> const String& (dropdown name)
unsafe extern "C" fn custom_get_short_name(_this: *mut u8) -> *const u8 {
    SHORT_NAME.as_ptr()
}

static mut CLONE_VTABLE: [u64; 128] = [0u64; 128];

#[unsafe(no_mangle)]
pub extern "C" fn dllStartPlugin() {
    let _ = std::fs::File::create(LOG_FILE);
    
    log("=========================================================");
    log("  KenshiReborn v0.8.0 — THE RENAME");
    log("  vtable[1]=getName, vtable[2]=shortName — swap BOTH!");
    log("=========================================================");
    
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
        
        log(&format!("  D3D11 RS at {:p}", d3d11_rs));
        log(&format!("  vtable[0] = 0x{:x} (destructor)", *d3d11_vtable.add(0)));
        log(&format!("  vtable[1] = 0x{:x} (getName)", *d3d11_vtable.add(1)));
        log(&format!("  vtable[2] = 0x{:x} (shortName/dropdown)", *d3d11_vtable.add(2)));
        
        // Initialize our names
        init_names();
        
        // ═══ CLONE D3D11 ═══
        let clone_size: usize = 8192;
        let clone = VirtualAlloc(ptr::null_mut(), clone_size, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
        if clone.is_null() { log("ERROR: VirtualAlloc failed"); return; }
        
        ptr::copy_nonoverlapping(d3d11_rs, clone, clone_size);
        log(&format!("  Clone at {:p}", clone));
        
        // ═══ CLONE VTABLE — swap slots 1 AND 2 ═══
        ptr::copy_nonoverlapping(d3d11_vtable as *const u8, CLONE_VTABLE.as_mut_ptr() as *mut u8, 128 * 8);
        CLONE_VTABLE[1] = custom_get_name as u64;       // getName -> "Vulkan (Reborn)"
        CLONE_VTABLE[2] = custom_get_short_name as u64;  // shortName -> "Vulkan Reborn"
        *(clone as *mut u64) = CLONE_VTABLE.as_ptr() as u64;
        
        log("  Vtable cloned:");
        log(&format!("    slot[1] = custom_get_name -> 'Vulkan (Reborn)'"));
        log(&format!("    slot[2] = custom_get_short_name -> 'Vulkan Reborn'"));
        
        // ═══ REGISTER ═══
        add_rs(root, clone);
        
        let vec2 = get_renderers(root);
        let count2 = ((*vec2).end as usize - (*vec2).begin as usize) / 8;
        log(&format!("  Renderers: {} -> {}", count, count2));
        
        log("");
        log("=========================================================");
        log("  v0.8.0: BOTH names swapped!");
        log("  Dropdown should show 'Vulkan Reborn' (vtable[2])");
        log("  getName returns 'Vulkan (Reborn)' (vtable[1])");
        log("=========================================================");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dllStopPlugin() {
    log("dllStopPlugin() v0.8.0");
}
