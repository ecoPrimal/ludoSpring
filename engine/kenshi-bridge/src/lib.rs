// KenshiReborn Bridge DLL — RenderSystem_Vulkan_x64.dll
// v0.8.1 — COEXIST: Minimal skeleton that doesn't break D3D11
//
// v0.8.0: "Vulkan Reborn" in dropdown! But BOTH crash on launch.
//         Clone shares D3D11 internal state → conflicts on init.
// v0.8.1: DON'T clone 8KB of D3D11 state. Instead:
//         - Allocate MINIMAL object (just vtable pointer + padding)
//         - Override ALL critical vtable slots with safe stubs
//         - D3D11 stays clean, Vulkan Reborn shows in dropdown
//         - Selecting Vulkan Reborn = safe no-op (doesn't crash)

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
// NAMES — SSO strings for vtable[1] and vtable[2]
// ═══════════════════════════════════════════════════════════

static mut LONG_NAME: [u8; 32] = [0u8; 32];   // "Vulkan (Reborn)" — 15 chars
static mut SHORT_NAME: [u8; 32] = [0u8; 32];  // "Vulkan Reborn"  — 13 chars
static NAME_INIT: std::sync::Once = std::sync::Once::new();

unsafe fn init_names() {
    NAME_INIT.call_once(|| {
        // Long: "Vulkan (Reborn)" = 15 chars, fits SSO
        let n = LONG_NAME.as_mut_ptr();
        ptr::copy_nonoverlapping(b"Vulkan (Reborn)\0".as_ptr(), n, 16);
        *(n.add(16) as *mut u64) = 15;
        *(n.add(24) as *mut u64) = 15;
        
        // Short: "Vulkan Reborn" = 13 chars, fits SSO
        let s = SHORT_NAME.as_mut_ptr();
        ptr::copy_nonoverlapping(b"Vulkan Reborn\0\0\0".as_ptr(), s, 16);
        *(s.add(16) as *mut u64) = 13;
        *(s.add(24) as *mut u64) = 15;
    });
}

// ═══════════════════════════════════════════════════════════
// EMPTY CONFIG MAP — MSVC std::map sentinel node
// ═══════════════════════════════════════════════════════════
// Empty std::map = sentinel node where _Left/_Parent/_Right = self, _Color=1, _Isnil=1
// This prevents crashes when the launcher reads config options

static mut EMPTY_MAP: [u8; 64] = [0u8; 64];
static MAP_INIT: std::sync::Once = std::sync::Once::new();

unsafe fn init_empty_map() {
    MAP_INIT.call_once(|| {
        let m = EMPTY_MAP.as_mut_ptr();
        let sentinel = m as u64;
        // _Left (offset 0) = self
        *(m as *mut u64) = sentinel;
        // _Parent (offset 8) = self
        *(m.add(8) as *mut u64) = sentinel;
        // _Right (offset 16) = self
        *(m.add(16) as *mut u64) = sentinel;
        // _Color (offset 24) = 1 (black)
        *m.add(24) = 1;
        // _Isnil (offset 25) = 1
        *m.add(25) = 1;
        // _Mysize (offset 32) = 0
        *(m.add(32) as *mut u64) = 0;
    });
}

// ═══════════════════════════════════════════════════════════
// VTABLE STUBS — Safe functions for every critical slot
// ═══════════════════════════════════════════════════════════

// slot[0]: destructor — no-op (don't free our static memory)
unsafe extern "C" fn stub_destructor(_this: *mut u8) {
    log("  [vtable] destructor called (no-op)");
}

// slot[1]: getName() -> const String& 
unsafe extern "C" fn stub_get_name(_this: *mut u8) -> *const u8 {
    LONG_NAME.as_ptr()
}

// slot[2]: short name / friendly name for dropdown
unsafe extern "C" fn stub_get_short_name(_this: *mut u8) -> *const u8 {
    SHORT_NAME.as_ptr()
}

// slot[3]: getConfigOptions() -> const ConfigOptionMap& (return empty map)
unsafe extern "C" fn stub_get_config_options(_this: *mut u8) -> *const u8 {
    EMPTY_MAP.as_ptr()
}

// slot[4]: setConfigOption(name, value) — no-op, just accept
unsafe extern "C" fn stub_set_config_option(_this: *mut u8, _name: *const u8, _val: *const u8) {
    log("  [vtable] setConfigOption (no-op)");
}

// slot[5]: validateConfigOptions() -> String (return empty)
unsafe extern "C" fn stub_validate_config(_this: *mut u8) -> *const u8 {
    // Return empty SSO string
    static mut EMPTY_STR: [u8; 32] = [0u8; 32];
    EMPTY_STR.as_ptr()
}

// Generic no-op that returns null/0 — safe fallback for any slot
unsafe extern "C" fn stub_noop(_this: *mut u8) -> u64 { 0 }

// For methods that return *this or a reference  
unsafe extern "C" fn stub_return_this(this: *mut u8) -> *mut u8 { this }

// For methods that return bool
unsafe extern "C" fn stub_return_false(_this: *mut u8) -> bool { false }

// For _initialise (called when user selects this renderer and clicks OK)
unsafe extern "C" fn stub_initialise(_this: *mut u8, _auto_window: bool, _title: *const u8) -> *mut u8 {
    log("  [vtable] _initialise called — Vulkan Reborn not ready yet, safe no-op");
    ptr::null_mut()
}

static mut OUR_VTABLE: [u64; 128] = [0u64; 128];

#[unsafe(no_mangle)]
pub extern "C" fn dllStartPlugin() {
    let _ = std::fs::File::create(LOG_FILE);
    
    log("=========================================================");
    log("  KenshiReborn v0.8.1 — COEXIST");
    log("  Minimal skeleton: D3D11 stays clean, Vulkan shows up");
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
        log(&format!("  Existing renderers: {}", count));
        
        // Get D3D11 vtable as template for slot layout
        if count > 0 {
            let d3d11_rs = *(*vec).begin;
            let d3d11_vtable = *(d3d11_rs as *const u64) as *const u64;
            log(&format!("  D3D11 RS at {:p}, vtable at {:p}", d3d11_rs, d3d11_vtable));
            
            // Copy D3D11 vtable as base (for any slots we don't override)
            // BUT we'll override all critical ones with safe stubs
            ptr::copy_nonoverlapping(d3d11_vtable as *const u8, OUR_VTABLE.as_mut_ptr() as *mut u8, 128 * 8);
        }
        
        // Initialize names and empty config map
        init_names();
        init_empty_map();
        
        // ═══ BUILD MINIMAL RS OBJECT ═══
        // Allocate zeroed memory — NO D3D11 state copied!
        let obj_size: usize = 8192;
        let obj = VirtualAlloc(ptr::null_mut(), obj_size, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
        if obj.is_null() { log("ERROR: VirtualAlloc failed"); return; }
        // Memory is zeroed by VirtualAlloc (MEM_COMMIT)
        
        // ═══ OVERRIDE CRITICAL VTABLE SLOTS ═══
        OUR_VTABLE[0] = stub_destructor as u64;         // destructor
        OUR_VTABLE[1] = stub_get_name as u64;           // getName -> "Vulkan (Reborn)"
        OUR_VTABLE[2] = stub_get_short_name as u64;     // shortName -> "Vulkan Reborn"
        OUR_VTABLE[3] = stub_get_config_options as u64;  // getConfigOptions -> empty map
        OUR_VTABLE[4] = stub_set_config_option as u64;   // setConfigOption -> no-op
        OUR_VTABLE[5] = stub_validate_config as u64;     // validateConfig -> empty string
        
        // Override likely init/shutdown slots with safe stubs
        // Slots 6-20 are various init, shutdown, createWindow etc.
        for i in 6..20 {
            OUR_VTABLE[i] = stub_noop as u64;
        }
        
        // Point object's vtable pointer to our vtable
        *(obj as *mut u64) = OUR_VTABLE.as_ptr() as u64;
        
        log(&format!("  Skeleton RS at {:p} (zeroed, {} bytes)", obj, obj_size));
        log("  Vtable slots 0-19 overridden with safe stubs");
        log("  No D3D11 state copied — clean coexistence!");
        
        // ═══ REGISTER ═══
        add_rs(root, obj);
        
        let vec2 = get_renderers(root);
        let count2 = ((*vec2).end as usize - (*vec2).begin as usize) / 8;
        log(&format!("  Renderers: {} -> {}", count, count2));
        
        log("");
        log("=========================================================");
        log("  v0.8.1 COEXIST:");
        log("  - 'Vulkan Reborn' in dropdown (vtable[2])");
        log("  - D3D11 untouched — should launch normally!");
        log("  - Selecting Vulkan Reborn = safe no-op");
        log("  - NEXT: bridge wgpu into the skeleton");
        log("=========================================================");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dllStopPlugin() {
    log("dllStopPlugin() v0.8.1 — coexistence holds");
}
