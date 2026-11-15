// KenshiReborn Bridge DLL — RenderSystem_Vulkan_x64.dll
// v0.8.2 — CLEAN: All 128 vtable slots safe, D3D11 can launch the game
//
// v0.8.1 BUG: Slots 20-127 still pointed to D3D11 functions but got
//   called with our zeroed object as `this` → crash on game launch.
// v0.8.2 FIX: Override ALL 128 slots with safe stubs. Zero D3D11 leakage.

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
// NAMES — MSVC SSO strings
// ═══════════════════════════════════════════════════════════

static mut LONG_NAME: [u8; 32] = [0u8; 32];
static mut SHORT_NAME: [u8; 32] = [0u8; 32];
static mut EMPTY_STR: [u8; 32] = [0u8; 32];
static NAME_INIT: std::sync::Once = std::sync::Once::new();

unsafe fn init_names() {
    NAME_INIT.call_once(|| {
        let n = LONG_NAME.as_mut_ptr();
        ptr::copy_nonoverlapping(b"Vulkan (Reborn)\0".as_ptr(), n, 16);
        *(n.add(16) as *mut u64) = 15;
        *(n.add(24) as *mut u64) = 15;

        let s = SHORT_NAME.as_mut_ptr();
        ptr::copy_nonoverlapping(b"Vulkan Reborn\0\0\0".as_ptr(), s, 16);
        *(s.add(16) as *mut u64) = 13;
        *(s.add(24) as *mut u64) = 15;
    });
}

// ═══════════════════════════════════════════════════════════
// EMPTY CONFIG MAP — MSVC std::map sentinel
// ═══════════════════════════════════════════════════════════

static mut EMPTY_MAP: [u8; 64] = [0u8; 64];
static MAP_INIT: std::sync::Once = std::sync::Once::new();

unsafe fn init_empty_map() {
    MAP_INIT.call_once(|| {
        let m = EMPTY_MAP.as_mut_ptr();
        let sentinel = m as u64;
        *(m as *mut u64) = sentinel;
        *(m.add(8) as *mut u64) = sentinel;
        *(m.add(16) as *mut u64) = sentinel;
        *m.add(24) = 1;
        *m.add(25) = 1;
        *(m.add(32) as *mut u64) = 0;
    });
}

// ═══════════════════════════════════════════════════════════
// VTABLE STUBS — every slot is safe
// ═══════════════════════════════════════════════════════════

unsafe extern "C" fn stub_noop(_this: *mut u8) -> u64 { 0 }
unsafe extern "C" fn stub_get_name(_this: *mut u8) -> *const u8 { LONG_NAME.as_ptr() }
unsafe extern "C" fn stub_get_short_name(_this: *mut u8) -> *const u8 { SHORT_NAME.as_ptr() }
unsafe extern "C" fn stub_get_config(_this: *mut u8) -> *const u8 { EMPTY_MAP.as_ptr() }
unsafe extern "C" fn stub_set_config(_this: *mut u8, _a: *const u8, _b: *const u8) {}
unsafe extern "C" fn stub_validate(_this: *mut u8) -> *const u8 { EMPTY_STR.as_ptr() }

static mut VTABLE: [u64; 128] = [0u64; 128];

#[unsafe(no_mangle)]
pub extern "C" fn dllStartPlugin() {
    let _ = std::fs::File::create(LOG_FILE);
    log("KenshiReborn v0.8.2 — CLEAN");
    
    unsafe {
        let ogre = GetModuleHandleA(b"OgreMain_x64.dll\0".as_ptr());
        if ogre.is_null() { log("  ERROR: OgreMain not found"); return; }
        
        let fn_get_root = GetProcAddress(ogre, SYM_ROOT_GET_SINGLETON.as_ptr());
        let fn_get_renderers = GetProcAddress(ogre, SYM_GET_AVAIL_RENDERERS.as_ptr());
        let fn_add_rs = GetProcAddress(ogre, SYM_ADD_RENDER_SYSTEM.as_ptr());
        
        if fn_get_root.is_null() || fn_get_renderers.is_null() || fn_add_rs.is_null() {
            log("  ERROR: symbols"); return;
        }
        
        let get_root: FnGetSingletonPtr = std::mem::transmute(fn_get_root);
        let get_renderers: FnGetAvailableRenderers = std::mem::transmute(fn_get_renderers);
        let add_rs: FnAddRenderSystem = std::mem::transmute(fn_add_rs);
        
        let root = get_root();
        if root.is_null() { log("  ERROR: null root"); return; }
        
        let vec = get_renderers(root);
        let count = ((*vec).end as usize - (*vec).begin as usize) / 8;
        
        init_names();
        init_empty_map();
        
        // ALL 128 slots → safe no-op (returns 0/null)
        for i in 0..128 {
            VTABLE[i] = stub_noop as u64;
        }
        
        // Override the slots that need specific returns
        VTABLE[1] = stub_get_name as u64;
        VTABLE[2] = stub_get_short_name as u64;
        VTABLE[3] = stub_get_config as u64;
        VTABLE[4] = stub_set_config as u64;
        VTABLE[5] = stub_validate as u64;
        
        // Allocate zeroed RS object
        let obj = VirtualAlloc(ptr::null_mut(), 8192, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
        if obj.is_null() { log("  ERROR: alloc"); return; }
        *(obj as *mut u64) = VTABLE.as_ptr() as u64;
        
        add_rs(root, obj);
        
        let vec2 = get_renderers(root);
        let count2 = ((*vec2).end as usize - (*vec2).begin as usize) / 8;
        log(&format!("  Renderers: {} -> {} | Vulkan Reborn ready", count, count2));
        log("  All 128 vtable slots safe. D3D11 untouched. Play on.");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dllStopPlugin() {}
