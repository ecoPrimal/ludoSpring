// KenshiReborn Bridge DLL — RenderSystem_Vulkan_x64.dll
// v0.7.1 — SAFE SIEGE: All vtable slots stubbed except getName
//
// v0.7.0 PROVED:
//   - getName is at vtable SLOT 1
//   - addRenderSystem works — we got 2 renderers!
//   - But the launcher crashes calling other methods on our fake object
//
// v0.7.1 FIX:
//   - Stub ALL 64 vtable entries with safe no-op functions
//   - Only slot 1 (getName) returns "Vulkan (Reborn)"
//   - Slot 0 (destructor) does nothing
//   - All other slots return 0/null/empty safely
//   - The launcher should see "Vulkan (Reborn)" in the dropdown
//     but selecting it won't initialize (that's Phase 3)

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

// Stub that returns null pointer (for methods returning pointers/references)
unsafe extern "C" fn stub_return_null(_this: *mut u8) -> *mut u8 {
    ptr::null_mut()
}

// Stub for getConfigOptions — returns pointer to a static empty ConfigOptionMap
// ConfigOptionMap is std::map<String, ConfigOption> — empty = 0-size node
// For MSVC std::map, an empty map has _Myhead (sentinel) and _Mysize = 0
// We fake an empty map: just return a zeroed region
static EMPTY_CONFIG_MAP: [u8; 64] = [0u8; 64];

unsafe extern "C" fn stub_get_config_options(_this: *mut u8) -> *const u8 {
    log("  [vtable] getConfigOptions called — returning empty");
    EMPTY_CONFIG_MAP.as_ptr()
}

// Stub for methods that take a String& parameter (like setConfigOption)
unsafe extern "C" fn stub_set_config(_this: *mut u8, _name: *const u8, _value: *const u8) {
    log("  [vtable] setConfigOption called — ignored");
}

// Stub for validateConfigOptions
unsafe extern "C" fn stub_validate(_this: *mut u8) -> *const u8 {
    log("  [vtable] validateConfigOptions called — returning empty string");
    // Return pointer to a valid empty MSVC string
    static EMPTY_STRING: [u8; 32] = [0u8; 32]; // SSO empty: all zeros = len 0, cap 0
    EMPTY_STRING.as_ptr()
}

// Track which slots get called so we can learn the launcher's behavior
static SLOT_CALL_LOG: AtomicPtr<u8> = AtomicPtr::new(ptr::null_mut());

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
    log("║  KenshiReborn v0.7.1 — SAFE SIEGE                   ║");
    log("║  All vtable stubs + getName = 'Vulkan (Reborn)'     ║");
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
        
        // Logging stubs for the first 20 slots to see what the launcher calls
        SAFE_VTABLE[2] = stub_slot_2 as *const u8;
        SAFE_VTABLE[3] = stub_slot_3 as *const u8;
        SAFE_VTABLE[4] = stub_slot_4 as *const u8;
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
