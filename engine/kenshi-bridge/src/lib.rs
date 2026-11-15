// KenshiReborn Bridge DLL — RenderSystem_Vulkan_x64.dll
// v0.7.2 — DOUGH RISING: Fix slot 2 (getConfigOptions)
//
// sourDough fermentation log:
//   v0.7.0: getName=slot1, addRS works, but crashes (stale D3D11 ptrs)
//   v0.7.1: All stubs, registered 2 renderers, slot 2 called → crash
//           Slot 2 returned 0 but Ogre dereferences it as ConfigOptionMap&
//   v0.7.2: Slot 2 returns valid empty ConfigOptionMap (MSVC std::map)
//           Slot 4 returns valid empty String (validateConfigOptions)
//           Log ALL slot calls to map the launcher's full call sequence

#![allow(non_snake_case)]

use std::fs::OpenOptions;
use std::io::Write;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};

const LOG_FILE: &str = r"M:\SteamLibrary\steamapps\common\Kenshi\kenshi_reborn.log";

fn log(msg: &str) {
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(LOG_FILE) {
        let _ = writeln!(f, "{}", msg);
    }
}

extern "system" {
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

const SYM_ROOT_GET_SINGLETON: &[u8] = b"?getSingletonPtr@Root@Ogre@@SAPEAV12@XZ\0";
const SYM_GET_AVAIL_RENDERERS: &[u8] = b"?getAvailableRenderers@Root@Ogre@@QEAAAEBV?$vector@PEAVRenderSystem@Ogre@@V?$STLAllocator@PEAVRenderSystem@Ogre@@V?$CategorisedAllocPolicy@$0A@@2@@2@@std@@XZ\0";
const SYM_ADD_RENDER_SYSTEM: &[u8] = b"?addRenderSystem@Root@Ogre@@QEAAXPEAVRenderSystem@2@@Z\0";

// ═══════════════════════════════════════════════════════════
// SAFE VTABLE: 64 stubs + getName
// ═══════════════════════════════════════════════════════════

// "Vulkan (Reborn)" as MSVC std::string in SSO mode (15 chars fits)
// Layout: [char data 0..15][padding to 16][size u64 = 15][capacity u64 = 15]
static VULKAN_NAME: &[u8; 32] = &{
    let mut buf = [0u8; 32];
    buf[0] = b'V'; buf[1] = b'u'; buf[2] = b'l'; buf[3] = b'k';
    buf[4] = b'a'; buf[5] = b'n'; buf[6] = b' '; buf[7] = b'(';
    buf[8] = b'R'; buf[9] = b'e'; buf[10] = b'b'; buf[11] = b'o';
    buf[12] = b'r'; buf[13] = b'n'; buf[14] = b')'; buf[15] = 0;
    // size = 15 at offset 16 (little-endian u64)
    buf[16] = 15; buf[17] = 0; buf[18] = 0; buf[19] = 0;
    buf[20] = 0; buf[21] = 0; buf[22] = 0; buf[23] = 0;
    // capacity = 15 at offset 24 (little-endian u64)
    buf[24] = 15; buf[25] = 0; buf[26] = 0; buf[27] = 0;
    buf[28] = 0; buf[29] = 0; buf[30] = 0; buf[31] = 0;
    buf
};

// Slot 0: Destructor — do nothing
unsafe extern "C" fn stub_destructor(_this: *mut u8, _flags: u32) -> *mut u8 {
    log("  [vtable] destructor called — no-op");
    _this
}

// Slot 1: getName — return "Vulkan (Reborn)"
unsafe extern "C" fn custom_get_name(_this: *mut u8) -> *const u8 {
    VULKAN_NAME.as_ptr()
}

// Generic stub that returns 0 (works for most bool/int/void returns)
unsafe extern "C" fn stub_return_zero(_this: *mut u8) -> u64 {
    0
}

// ═══════════════════════════════════════════════════════════
// MSVC std::map<String, ConfigOption> layout (empty map)
// An MSVC std::map has: _Myhead (sentinel node ptr), _Mysize (0)
// The sentinel node is a _Tree_node with: _Left, _Parent, _Right, _Color, _Isnil
// For an EMPTY map: _Myhead points to sentinel, sentinel._Left = sentinel._Right = sentinel
//                   sentinel._Isnil = true, _Mysize = 0
// ═══════════════════════════════════════════════════════════

// Sentinel node for empty std::map (must be self-referential)
// _Tree_node layout (MSVC x64): [_Left:8][_Parent:8][_Right:8][_Color:1][_Isnil:1][pad:6][_Myval:...]
// For sentinel: _Left = self, _Parent = self, _Right = self, _Color = 1 (black), _Isnil = 1
static mut MAP_SENTINEL: [u8; 64] = [0u8; 64];
static MAP_SENTINEL_INIT: std::sync::Once = std::sync::Once::new();

// The std::map object itself: [_Myhead:8 (ptr to sentinel)][_Mysize:8 (= 0)]
static mut EMPTY_MAP: [u8; 16] = [0u8; 16];
static MAP_INIT: std::sync::Once = std::sync::Once::new();

unsafe fn init_empty_map() -> *const u8 {
    MAP_INIT.call_once(|| {
        // Initialize sentinel to point to itself
        let sentinel_ptr = MAP_SENTINEL.as_mut_ptr();
        let sentinel_addr = sentinel_ptr as u64;
        
        // _Left = self (offset 0)
        *(sentinel_ptr as *mut u64) = sentinel_addr;
        // _Parent = self (offset 8)
        *(sentinel_ptr.add(8) as *mut u64) = sentinel_addr;
        // _Right = self (offset 16)
        *(sentinel_ptr.add(16) as *mut u64) = sentinel_addr;
        // _Color = 1 (black) at offset 24
        *sentinel_ptr.add(24) = 1;
        // _Isnil = 1 (true) at offset 25
        *sentinel_ptr.add(25) = 1;
        
        // Initialize the map object
        let map_ptr = EMPTY_MAP.as_mut_ptr();
        // _Myhead = pointer to sentinel (offset 0)
        *(map_ptr as *mut u64) = sentinel_addr;
        // _Mysize = 0 (offset 8) — already zeroed
    });
    EMPTY_MAP.as_ptr()
}

// Slot 2: getConfigOptions() -> ConfigOptionMap&
// Returns reference (pointer) to our static empty map
unsafe extern "C" fn stub_get_config_options(_this: *mut u8) -> *const u8 {
    log("  [vtable] slot 2: getConfigOptions → empty map");
    init_empty_map()
}

// Slot 3: setConfigOption(name, value) -> void
unsafe extern "C" fn stub_set_config(_this: *mut u8, _name: *const u8, _value: *const u8) {
    log("  [vtable] slot 3: setConfigOption → ignored");
}

// Slot 4: validateConfigOptions() -> String
// Returns pointer to a valid empty MSVC std::string
static EMPTY_STRING: [u8; 32] = [0u8; 32];

unsafe extern "C" fn stub_validate(_this: *mut u8) -> *const u8 {
    log("  [vtable] slot 4: validateConfigOptions → empty string");
    EMPTY_STRING.as_ptr()
}

macro_rules! make_logging_stub {
    ($name:ident, $slot:expr) => {
        unsafe extern "C" fn $name(_this: *mut u8) -> u64 {
            log(&format!("  [vtable] slot {} called", $slot));
            0
        }
    };
}

// Generate logging stubs for interesting slots
make_logging_stub!(stub_slot_2, 2);
make_logging_stub!(stub_slot_3, 3);
make_logging_stub!(stub_slot_4, 4);
make_logging_stub!(stub_slot_5, 5);
make_logging_stub!(stub_slot_6, 6);
make_logging_stub!(stub_slot_7, 7);
make_logging_stub!(stub_slot_8, 8);
make_logging_stub!(stub_slot_9, 9);
make_logging_stub!(stub_slot_10, 10);
make_logging_stub!(stub_slot_11, 11);
make_logging_stub!(stub_slot_12, 12);
make_logging_stub!(stub_slot_13, 13);
make_logging_stub!(stub_slot_14, 14);
make_logging_stub!(stub_slot_15, 15);
make_logging_stub!(stub_slot_16, 16);
make_logging_stub!(stub_slot_17, 17);
make_logging_stub!(stub_slot_18, 18);
make_logging_stub!(stub_slot_19, 19);
make_logging_stub!(stub_slot_20, 20);

// Fake RenderSystem object — 8KB zeroed with our vtable pointer at offset 0
// and "Vulkan (Reborn)" string at offset 8
static mut FAKE_RS: [u8; 8192] = [0u8; 8192];
static mut SAFE_VTABLE: [*const u8; 64] = [ptr::null(); 64];

#[unsafe(no_mangle)]
pub extern "C" fn dllStartPlugin() {
    let _ = std::fs::File::create(LOG_FILE);
    
    log("╔═══════════════════════════════════════════════════════╗");
    log("║  KenshiReborn v0.7.2 — DOUGH RISING                  ║");
    log("║  Slot 2 = empty ConfigOptionMap, getName works      ║");
    log("╚═══════════════════════════════════════════════════════╝");
    
    unsafe {
        let ogre = GetModuleHandleA(b"OgreMain_x64.dll\0".as_ptr());
        if ogre.is_null() {
            log("ERROR: OgreMain_x64.dll not found");
            return;
        }
        log(&format!("  OgreMain at {:?}", ogre));
        
        let fn_get_root = GetProcAddress(ogre, SYM_ROOT_GET_SINGLETON.as_ptr());
        let fn_get_renderers = GetProcAddress(ogre, SYM_GET_AVAIL_RENDERERS.as_ptr());
        let fn_add_rs = GetProcAddress(ogre, SYM_ADD_RENDER_SYSTEM.as_ptr());
        
        if fn_get_root.is_null() || fn_get_renderers.is_null() || fn_add_rs.is_null() {
            log("ERROR: Symbol resolution failed");
            return;
        }
        
        let get_root: FnGetSingletonPtr = std::mem::transmute(fn_get_root);
        let get_renderers: FnGetAvailableRenderers = std::mem::transmute(fn_get_renderers);
        let add_rs: FnAddRenderSystem = std::mem::transmute(fn_add_rs);
        
        let root = get_root();
        if root.is_null() {
            log("ERROR: Root is null");
            return;
        }
        
        let vec = get_renderers(root);
        let count = ((*vec).end as usize - (*vec).begin as usize) / 8;
        log(&format!("  {} existing renderer(s)", count));
        
        // ═══ BUILD SAFE VTABLE ═══
        // Fill ALL 64 slots with safe stubs first
        for i in 0..64 {
            SAFE_VTABLE[i] = stub_return_zero as *const u8;
        }
        
        // Slot 0: safe destructor (returns this, does nothing)
        SAFE_VTABLE[0] = stub_destructor as *const u8;
        
        // Slot 1: getName → "Vulkan (Reborn)" ★
        SAFE_VTABLE[1] = custom_get_name as *const u8;
        
        // Proper implementations for slots 2-4 (launcher needs these)
        SAFE_VTABLE[2] = stub_get_config_options as *const u8; // getConfigOptions → empty map
        SAFE_VTABLE[3] = stub_set_config as *const u8;         // setConfigOption → ignore
        SAFE_VTABLE[4] = stub_validate as *const u8;           // validateConfigOptions → empty string
        SAFE_VTABLE[5] = stub_slot_5 as *const u8;
        SAFE_VTABLE[6] = stub_slot_6 as *const u8;
        SAFE_VTABLE[7] = stub_slot_7 as *const u8;
        SAFE_VTABLE[8] = stub_slot_8 as *const u8;
        SAFE_VTABLE[9] = stub_slot_9 as *const u8;
        SAFE_VTABLE[10] = stub_slot_10 as *const u8;
        SAFE_VTABLE[11] = stub_slot_11 as *const u8;
        SAFE_VTABLE[12] = stub_slot_12 as *const u8;
        SAFE_VTABLE[13] = stub_slot_13 as *const u8;
        SAFE_VTABLE[14] = stub_slot_14 as *const u8;
        SAFE_VTABLE[15] = stub_slot_15 as *const u8;
        SAFE_VTABLE[16] = stub_slot_16 as *const u8;
        SAFE_VTABLE[17] = stub_slot_17 as *const u8;
        SAFE_VTABLE[18] = stub_slot_18 as *const u8;
        SAFE_VTABLE[19] = stub_slot_19 as *const u8;
        SAFE_VTABLE[20] = stub_slot_20 as *const u8;
        
        log("  Built safe vtable (64 stubs + getName at slot 1)");
        
        // ═══ BUILD FAKE RENDERSYSTEM ═══
        // Zero it all out first (already static zeroed)
        let fake_rs_ptr = FAKE_RS.as_mut_ptr();
        
        // Byte 0-7: vtable pointer
        *(fake_rs_ptr as *mut *const *const u8) = SAFE_VTABLE.as_ptr() as *const *const u8;
        
        // Byte 8-39: mName = "Vulkan (Reborn)" as MSVC SSO string
        ptr::copy_nonoverlapping(VULKAN_NAME.as_ptr(), fake_rs_ptr.add(8), 32);
        
        log("  Built fake RenderSystem (zeroed + vtable + mName)");
        
        // ═══ REGISTER ═══
        add_rs(root, fake_rs_ptr);
        
        let vec2 = get_renderers(root);
        let count2 = ((*vec2).end as usize - (*vec2).begin as usize) / 8;
        log(&format!("  Renderers: {} → {} ", count, count2));
        
        if count2 > count {
            log("");
            log("╔═══════════════════════════════════════════════════════╗");
            log("║  ★ 'Vulkan (Reborn)' REGISTERED — SAFE SIEGE ★      ║");
            log("║  All methods stubbed. Dropdown should show it.       ║");
            log("║  Selecting it will be a no-op (Phase 3 handles that) ║");
            log("╚═══════════════════════════════════════════════════════╝");
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dllStopPlugin() {
    log("dllStopPlugin() — safe siege standing down");
}
