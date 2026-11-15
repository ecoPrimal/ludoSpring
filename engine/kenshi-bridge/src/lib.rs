// KenshiReborn Bridge DLL — RenderSystem_Vulkan_x64.dll
// v0.7.4 — LOCUST SWARM: Self-referential safe memory everywhere
//
// sourDough log:
//   v0.7.0: getName ✓, 2 renderers ✓, crashed (D3D11 ptrs)
//   v0.7.1: all stubs, slot 2 crash (returned 0)
//   v0.7.2: slot 2 empty map ✓, slot 3 ✓, crashed after
//   v0.7.3: all 64 slots handled, STILL crashes after slot 3
//   v0.7.4: LOCUST SWARM — every byte of the fake RS is safe to deref
//           The fake object is a 64KB pool of self-referential pointers.
//           ANY chain of pointer reads → valid memory → zeros → safe.
//           The launcher TOUCHES all renderers to build UI, we survive.

#![allow(non_snake_case, unused)]

use std::fs::OpenOptions;
use std::io::Write;
use std::ptr;
use std::sync::Once;

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
// THE SAFE POOL — 64KB of self-referential memory
// Every 8-byte slot points BACK into this pool.
// Any chain of dereferences always lands on valid memory.
// ═══════════════════════════════════════════════════════════

const POOL_SIZE: usize = 65536;
static mut SAFE_POOL: [u8; POOL_SIZE] = [0u8; POOL_SIZE];
static POOL_INIT: Once = Once::new();

unsafe fn init_pool() -> *mut u8 {
    let pool = SAFE_POOL.as_mut_ptr();
    let pool_addr = pool as u64;
    
    // Fill entire pool with pointer-to-pool at every 8-byte boundary
    // So reading any 8-byte aligned offset gives a pointer back to pool
    // Creating an infinite safe dereference chain
    let slots = POOL_SIZE / 8;
    for i in 0..slots {
        let slot = pool.add(i * 8) as *mut u64;
        // Point to a region 256 bytes into the pool (avoids vtable at 0)
        // This region is also full of self-pointers, so it's safe to deref
        *slot = pool_addr + 256;
    }
    
    pool
}

// ═══════════════════════════════════════════════════════════
// MSVC std::string SSO layout
// ═══════════════════════════════════════════════════════════

const STR_OFFSET: usize = 4096; // Name string at pool+4096

unsafe fn write_sso_string(base: *mut u8, text: &[u8]) {
    ptr::copy_nonoverlapping(text.as_ptr(), base, text.len());
    *base.add(text.len()) = 0; // null term
    *(base.add(16) as *mut u64) = text.len() as u64; // size
    *(base.add(24) as *mut u64) = 15; // capacity (SSO)
}

unsafe fn write_empty_string(base: *mut u8) {
    ptr::write_bytes(base, 0, 32);
}

// ═══════════════════════════════════════════════════════════
// MSVC std::map — self-referential empty sentinel
// ═══════════════════════════════════════════════════════════

const MAP_SENTINEL_OFFSET: usize = 8192;
const MAP_OFFSET: usize = 8256;

unsafe fn init_empty_map(pool: *mut u8) {
    let s = pool.add(MAP_SENTINEL_OFFSET);
    let s_addr = s as u64;
    
    *(s as *mut u64) = s_addr;          // _Left = self
    *(s.add(8) as *mut u64) = s_addr;   // _Parent = self
    *(s.add(16) as *mut u64) = s_addr;  // _Right = self
    *s.add(24) = 1;                     // _Color = black
    *s.add(25) = 1;                     // _Isnil = true
    
    let m = pool.add(MAP_OFFSET);
    *(m as *mut u64) = s_addr;          // _Myhead = sentinel
    *(m.add(8) as *mut u64) = 0;        // _Mysize = 0
}

// ═══════════════════════════════════════════════════════════
// VTABLE — 64 safe slots, each logged for the first few
// ═══════════════════════════════════════════════════════════

const VTABLE_OFFSET: usize = 16384;

// Slot 0: Destructor
unsafe extern "C" fn vt_destructor(this: *mut u8, _flags: u32) -> *mut u8 { this }

// Slot 1: getName → const String& "Vulkan (Reborn)"
unsafe extern "C" fn vt_get_name(_this: *mut u8) -> *const u8 {
    SAFE_POOL.as_ptr().add(STR_OFFSET)
}

// Slot 2: getConfigOptions → ConfigOptionMap& (empty)
unsafe extern "C" fn vt_get_config_options(_this: *mut u8) -> *const u8 {
    log("  [vt2] getConfigOptions → empty map");
    SAFE_POOL.as_ptr().add(MAP_OFFSET)
}

// Slot 3: setConfigOption → void
unsafe extern "C" fn vt_set_config_option(_this: *mut u8, _name: *const u8, _val: *const u8) {
    log("  [vt3] setConfigOption → ignored");
}

// Slot 4: validateConfigOptions → String (empty = valid)
unsafe extern "C" fn vt_validate_config(_this: *mut u8) -> *const u8 {
    log("  [vt4] validateConfigOptions → empty string");
    SAFE_POOL.as_ptr().add(STR_OFFSET + 64)  // empty string at +64 past name
}

// Slot 5: initialise / createWindow — big function, return null (not used)
unsafe extern "C" fn vt_slot5(this: *mut u8, a: u64, b: u64, c: u64, d: u64) -> u64 {
    log("  [vt5] initialise/createWindow → 0");
    0
}

// Slot 6: getPriority → 0
unsafe extern "C" fn vt_slot6(_this: *mut u8) -> u64 {
    log("  [vt6] getPriority → 0");
    0
}

// Generic logged stub — returns pointer to safe pool region
unsafe extern "C" fn vt_safe_logged(this: *mut u8) -> *const u8 {
    // Try to determine which slot called us by scanning call stack
    // For now just return safe memory
    SAFE_POOL.as_ptr().add(256)
}

// Generic stub for slots 21+ — silent, returns safe pointer
unsafe extern "C" fn vt_safe_silent(_this: *mut u8) -> *const u8 {
    SAFE_POOL.as_ptr().add(256)
}

// Individual logged stubs for slots 7-20 so we can see which fires
macro_rules! make_logged_slot {
    ($fname:ident, $n:expr) => {
        unsafe extern "C" fn $fname(_this: *mut u8) -> *const u8 {
            log(concat!("  [vt", stringify!($n), "] called → safe ptr"));
            SAFE_POOL.as_ptr().add(256)
        }
    };
}

make_logged_slot!(vt_slot7, 7);
make_logged_slot!(vt_slot8, 8);
make_logged_slot!(vt_slot9, 9);
make_logged_slot!(vt_slot10, 10);
make_logged_slot!(vt_slot11, 11);
make_logged_slot!(vt_slot12, 12);
make_logged_slot!(vt_slot13, 13);
make_logged_slot!(vt_slot14, 14);
make_logged_slot!(vt_slot15, 15);
make_logged_slot!(vt_slot16, 16);
make_logged_slot!(vt_slot17, 17);
make_logged_slot!(vt_slot18, 18);
make_logged_slot!(vt_slot19, 19);
make_logged_slot!(vt_slot20, 20);

// ═══════════════════════════════════════════════════════════
// ASSEMBLY — Wire everything together
// ═══════════════════════════════════════════════════════════

#[unsafe(no_mangle)]
pub extern "C" fn dllStartPlugin() {
    let _ = std::fs::File::create(LOG_FILE);
    
    log("╔═══════════════════════════════════════════════════════╗");
    log("║  KenshiReborn v0.7.4 — LOCUST SWARM                 ║");
    log("║  64KB safe pool, self-referential, infinite deref    ║");
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
        
        // ═══ INIT THE SAFE POOL ═══
        let pool = init_pool();
        let pool_addr = pool as u64;
        log(&format!("  Safe pool at 0x{:x} (64KB, self-referential)", pool_addr));
        
        // Write "Vulkan (Reborn)" string at STR_OFFSET
        write_sso_string(pool.add(STR_OFFSET), b"Vulkan (Reborn)");
        
        // Write empty string at STR_OFFSET+64
        write_empty_string(pool.add(STR_OFFSET + 64));
        
        // Build empty ConfigOptionMap at MAP_OFFSET
        init_empty_map(pool);
        
        // ═══ BUILD VTABLE at VTABLE_OFFSET ═══
        let vtable = pool.add(VTABLE_OFFSET) as *mut *const u8;
        
        // Default all 64 slots to safe silent stub
        for i in 0..64 {
            *vtable.add(i) = vt_safe_silent as *const u8;
        }
        
        // Specific implementations
        *vtable.add(0)  = vt_destructor as *const u8;
        *vtable.add(1)  = vt_get_name as *const u8;
        *vtable.add(2)  = vt_get_config_options as *const u8;
        *vtable.add(3)  = vt_set_config_option as *const u8;
        *vtable.add(4)  = vt_validate_config as *const u8;
        *vtable.add(5)  = vt_slot5 as *const u8;
        *vtable.add(6)  = vt_slot6 as *const u8;
        *vtable.add(7)  = vt_slot7 as *const u8;
        *vtable.add(8)  = vt_slot8 as *const u8;
        *vtable.add(9)  = vt_slot9 as *const u8;
        *vtable.add(10) = vt_slot10 as *const u8;
        *vtable.add(11) = vt_slot11 as *const u8;
        *vtable.add(12) = vt_slot12 as *const u8;
        *vtable.add(13) = vt_slot13 as *const u8;
        *vtable.add(14) = vt_slot14 as *const u8;
        *vtable.add(15) = vt_slot15 as *const u8;
        *vtable.add(16) = vt_slot16 as *const u8;
        *vtable.add(17) = vt_slot17 as *const u8;
        *vtable.add(18) = vt_slot18 as *const u8;
        *vtable.add(19) = vt_slot19 as *const u8;
        *vtable.add(20) = vt_slot20 as *const u8;
        
        log("  Vtable: slots 0-4 proper, 5-20 logged, 21-63 safe");
        
        // ═══ BUILD FAKE RENDERSYSTEM ═══
        // The RS object itself starts at pool+0
        // Already filled with self-referential pointers by init_pool()
        // Now overlay the critical fields:
        
        let rs = pool;
        
        // [0] = vtable pointer (points to our vtable in the pool)
        *(rs as *mut u64) = (pool_addr + VTABLE_OFFSET as u64);
        
        // [8..39] = mName = "Vulkan (Reborn)" — SSO string
        ptr::copy_nonoverlapping(
            pool.add(STR_OFFSET),
            rs.add(8),
            32
        );
        
        // [40..55] = mConfigOptions (embedded empty map)
        ptr::copy_nonoverlapping(
            pool.add(MAP_OFFSET),
            rs.add(40),
            16
        );
        
        // [56..87] = Another empty string (mDescription or similar)
        write_empty_string(rs.add(56));
        
        // CRITICAL: All other fields (88+) are ALREADY safe pool pointers
        // from init_pool(). So any access at any offset → valid ptr → pool+256
        // → which itself is full of pool pointers → infinite safe chain.
        
        log("  Fake RS: vtable + mName + ConfigOptionMap + 64KB safe body");
        
        // ═══ REGISTER ═══
        add_rs(root, rs);
        
        let vec2 = get_renderers(root);
        let count2 = ((*vec2).end as usize - (*vec2).begin as usize) / 8;
        log(&format!("  Renderers: {} → {}", count, count2));
        
        if count2 > count {
            log("");
            log("╔═══════════════════════════════════════════════════════╗");
            log("║  ★ LOCUST SWARM DEPLOYED — ALL MEMORY SAFE ★        ║");
            log("║  64KB pool, infinite deref chain, NO null crashes.   ║");
            log("║  The 5090 will see 'Vulkan (Reborn)' in the list.   ║");
            log("╚═══════════════════════════════════════════════════════╝");
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dllStopPlugin() {
    log("dllStopPlugin() — swarm standing down");
}
