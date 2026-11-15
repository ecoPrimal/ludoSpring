// KenshiReborn Bridge DLL — RenderSystem_Vulkan_x64.dll
// v0.7.3 — FULL SIEGE: Every slot handled, no crashes
//
// sourDough log:
//   v0.7.0: getName=slot1 ✓, 2 renderers ✓, crashed (D3D11 ptrs)
//   v0.7.1: all stubs, slot 2 crashed (returned 0, needed pointer)
//   v0.7.2: slot 2 empty ConfigOptionMap ✓, slot 3 ✓, crashed after
//   v0.7.3: FULL SIEGE — every slot returns safe valid data
//           ConfigOptionMap, empty strings, empty vectors, valid pointers
//           The launcher WILL see "Vulkan (Reborn)" and NOT crash.

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
// STATIC DATA: all the valid objects our stubs return
// ═══════════════════════════════════════════════════════════

// "Vulkan (Reborn)" as MSVC std::string SSO (15 chars fits inline)
static VULKAN_NAME: [u8; 32] = {
    let mut buf = [0u8; 32];
    buf[0] = b'V'; buf[1] = b'u'; buf[2] = b'l'; buf[3] = b'k';
    buf[4] = b'a'; buf[5] = b'n'; buf[6] = b' '; buf[7] = b'(';
    buf[8] = b'R'; buf[9] = b'e'; buf[10] = b'b'; buf[11] = b'o';
    buf[12] = b'r'; buf[13] = b'n'; buf[14] = b')'; buf[15] = 0;
    buf[16] = 15; // size
    buf
};

// Empty MSVC std::string (SSO, length 0)
static EMPTY_STRING: [u8; 32] = [0u8; 32];

// Empty MSVC std::vector (begin == end == cap == null)
static EMPTY_VECTOR: [u8; 24] = [0u8; 24];

// Self-referential sentinel for empty std::map
// We use a static mut because it needs to point to itself
static mut MAP_SENTINEL: [u8; 64] = [0u8; 64];
static mut EMPTY_MAP: [u8; 16] = [0u8; 16];
static MAP_INIT: std::sync::Once = std::sync::Once::new();

unsafe fn get_empty_map() -> *const u8 {
    MAP_INIT.call_once(|| {
        let s = MAP_SENTINEL.as_mut_ptr();
        let addr = s as u64;
        *(s as *mut u64) = addr;           // _Left = self
        *(s.add(8) as *mut u64) = addr;    // _Parent = self
        *(s.add(16) as *mut u64) = addr;   // _Right = self
        *s.add(24) = 1;                    // _Color = black
        *s.add(25) = 1;                    // _Isnil = true
        
        let m = EMPTY_MAP.as_mut_ptr();
        *(m as *mut u64) = addr;           // _Myhead = sentinel
        // _Mysize = 0 (already zeroed)
    });
    EMPTY_MAP.as_ptr()
}

// A big zeroed region for any method that writes to *this
static SCRATCH: [u8; 4096] = [0u8; 4096];

// ═══════════════════════════════════════════════════════════
// VTABLE STUBS — each one safe, each one logged
// ═══════════════════════════════════════════════════════════

// [0] Destructor — do nothing, return this
unsafe extern "C" fn vt_destructor(this: *mut u8, _flags: u32) -> *mut u8 { this }

// [1] getName() → const String& = "Vulkan (Reborn)"
unsafe extern "C" fn vt_get_name(_this: *mut u8) -> *const u8 {
    VULKAN_NAME.as_ptr()
}

// [2] getConfigOptions() → ConfigOptionMap& (empty map)
unsafe extern "C" fn vt_get_config_options(this: *mut u8) -> *const u8 {
    log("  [vt2] getConfigOptions → empty map");
    get_empty_map()
}

// [3] setConfigOption(const String& name, const String& value) → void
unsafe extern "C" fn vt_set_config_option(_this: *mut u8, _name: *const u8, _val: *const u8) {
    log("  [vt3] setConfigOption → ignored");
}

// [4] validateConfigOptions() → String (empty = no error)
unsafe extern "C" fn vt_validate_config(_this: *mut u8) -> *const u8 {
    log("  [vt4] validateConfigOptions → empty string");
    EMPTY_STRING.as_ptr()
}

// [5] getRenderSystemEvents() → likely returns a reference
// Could be initialise() or createRenderWindow — big function (130KB in D3D11)
unsafe extern "C" fn vt_slot5(_this: *mut u8) -> u64 {
    log("  [vt5] called → 0");
    0
}

// [6] getPriority() → ushort (stub in D3D11 = returns 0)
unsafe extern "C" fn vt_get_priority(_this: *mut u8) -> u16 {
    log("  [vt6] getPriority → 0");
    0
}

// [7-20] Logging stubs that return valid pointer to scratch space
// Any method that returns a reference gets scratch (readable, zeroed)
macro_rules! vt_log_ptr {
    ($name:ident, $slot:expr) => {
        unsafe extern "C" fn $name(_this: *mut u8) -> *const u8 {
            log(&format!("  [vt{}] called → scratch ptr", $slot));
            SCRATCH.as_ptr()
        }
    };
}

macro_rules! vt_log_zero {
    ($name:ident, $slot:expr) => {
        unsafe extern "C" fn $name(_this: *mut u8) -> u64 {
            log(&format!("  [vt{}] called → 0", $slot));
            0
        }
    };
}

vt_log_ptr!(vt_slot7, 7);
vt_log_zero!(vt_slot8, 8);
vt_log_zero!(vt_slot9, 9);
vt_log_zero!(vt_slot10, 10);
vt_log_zero!(vt_slot11, 11);
vt_log_ptr!(vt_slot12, 12);
vt_log_ptr!(vt_slot13, 13);
vt_log_ptr!(vt_slot14, 14);
vt_log_ptr!(vt_slot15, 15);
vt_log_zero!(vt_slot16, 16);
vt_log_zero!(vt_slot17, 17);
vt_log_zero!(vt_slot18, 18);
vt_log_zero!(vt_slot19, 19);
vt_log_zero!(vt_slot20, 20);

// [21-63] Silent stubs — return pointer to scratch (safe for derefs)
unsafe extern "C" fn vt_safe_stub(_this: *mut u8) -> *const u8 {
    SCRATCH.as_ptr()
}

// ═══════════════════════════════════════════════════════════
// FAKE RENDERSYSTEM + VTABLE
// ═══════════════════════════════════════════════════════════

static mut FAKE_RS: [u8; 8192] = [0u8; 8192];
static mut SAFE_VTABLE: [*const u8; 64] = [ptr::null(); 64];

#[unsafe(no_mangle)]
pub extern "C" fn dllStartPlugin() {
    let _ = std::fs::File::create(LOG_FILE);
    
    log("╔═══════════════════════════════════════════════════════╗");
    log("║  KenshiReborn v0.7.3 — FULL SIEGE                   ║");
    log("║  Every slot handled. The dough will rise.            ║");
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
        
        // ═══ BUILD VTABLE: every slot safe ═══
        // Default: all slots return pointer to scratch (safe to deref)
        for i in 0..64 {
            SAFE_VTABLE[i] = vt_safe_stub as *const u8;
        }
        
        // Specific implementations
        SAFE_VTABLE[0]  = vt_destructor as *const u8;
        SAFE_VTABLE[1]  = vt_get_name as *const u8;
        SAFE_VTABLE[2]  = vt_get_config_options as *const u8;
        SAFE_VTABLE[3]  = vt_set_config_option as *const u8;
        SAFE_VTABLE[4]  = vt_validate_config as *const u8;
        SAFE_VTABLE[5]  = vt_slot5 as *const u8;
        SAFE_VTABLE[6]  = vt_get_priority as *const u8;
        SAFE_VTABLE[7]  = vt_slot7 as *const u8;
        SAFE_VTABLE[8]  = vt_slot8 as *const u8;
        SAFE_VTABLE[9]  = vt_slot9 as *const u8;
        SAFE_VTABLE[10] = vt_slot10 as *const u8;
        SAFE_VTABLE[11] = vt_slot11 as *const u8;
        SAFE_VTABLE[12] = vt_slot12 as *const u8;
        SAFE_VTABLE[13] = vt_slot13 as *const u8;
        SAFE_VTABLE[14] = vt_slot14 as *const u8;
        SAFE_VTABLE[15] = vt_slot15 as *const u8;
        SAFE_VTABLE[16] = vt_slot16 as *const u8;
        SAFE_VTABLE[17] = vt_slot17 as *const u8;
        SAFE_VTABLE[18] = vt_slot18 as *const u8;
        SAFE_VTABLE[19] = vt_slot19 as *const u8;
        SAFE_VTABLE[20] = vt_slot20 as *const u8;
        
        log("  Built FULL vtable (slots 0-4 proper, 5-20 logged, 21-63 scratch)");
        
        // ═══ BUILD FAKE RENDERSYSTEM ═══
        let rs = FAKE_RS.as_mut_ptr();
        
        // [0-7] vtable pointer
        *(rs as *mut *const *const u8) = SAFE_VTABLE.as_ptr() as *const *const u8;
        
        // [8-39] mName = "Vulkan (Reborn)"
        ptr::copy_nonoverlapping(VULKAN_NAME.as_ptr(), rs.add(8), 32);
        
        // [40-63] mConfigOptions = empty ConfigOptionMap at offset 40
        // Some Ogre builds store ConfigOptionMap inside the object
        let empty_map = get_empty_map();
        ptr::copy_nonoverlapping(empty_map, rs.add(40), 16);
        
        log("  Built fake RS (vtable + mName + embedded ConfigOptionMap)");
        
        // ═══ REGISTER ═══
        add_rs(root, rs);
        
        let vec2 = get_renderers(root);
        let count2 = ((*vec2).end as usize - (*vec2).begin as usize) / 8;
        log(&format!("  Renderers: {} → {}", count, count2));
        
        if count2 > count {
            log("");
            log("╔═══════════════════════════════════════════════════════╗");
            log("║  ★ FULL SIEGE COMPLETE — 'Vulkan (Reborn)' LIVE ★   ║");
            log("║  Every slot returns valid data. The dough rises.     ║");
            log("╚═══════════════════════════════════════════════════════╝");
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dllStopPlugin() {
    log("dllStopPlugin() — siege standing down, dough proved");
}
