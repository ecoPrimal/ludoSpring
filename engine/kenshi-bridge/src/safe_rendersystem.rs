// safe_rendersystem.rs — Build a crash-proof RenderSystem by cloning D3D11's vtable
//
// STRATEGY: After the investigator maps the D3D11 vtable, we:
// 1. Clone the ENTIRE D3D11 vtable to a new allocation
// 2. Replace ONLY the getName slot with our custom function
// 3. Build a fresh base RenderSystem object
// 4. Point its vtable to our cloned+patched vtable
// 5. Register with Root::addRenderSystem
//
// This way, ALL virtual methods use D3D11's real implementations,
// so Ogre can call getConfigOptions, validateConfig, etc. without crashing.
// The only difference: getName() returns "Vulkan (Reborn)" instead of "Direct3D11..."
//
// When the user actually SELECTS our renderer, Kenshi would try to init D3D11
// through our object — which will fail (no D3D11 device state). But at least
// the dropdown WORKS and the launcher doesn't crash.
//
// For a true Vulkan renderer, we'd need to implement all methods. But showing
// in the dropdown is the proof of concept.

use std::ptr;

/// Clone a vtable: copy `count` function pointers to a new heap allocation
pub unsafe fn clone_vtable(src_vtable: *const *const u8, count: usize) -> *mut *const u8 {
    let layout = std::alloc::Layout::from_size_align(count * 8, 8).unwrap();
    let dst = std::alloc::alloc(layout) as *mut *const u8;
    if dst.is_null() {
        return ptr::null_mut();
    }
    ptr::copy_nonoverlapping(src_vtable, dst, count);
    dst
}

/// Our custom getName implementation
/// Returns a pointer to a static MSVC std::string containing "Vulkan (Reborn)"
/// 
/// MSVC std::string layout (32 bytes):
///   [0..15]  SSO buffer (or heap ptr + padding)
///   [16..23] _Mysize
///   [24..31] _Myres (15 = SSO)
static mut VULKAN_NAME_STRING: [u8; 32] = [0u8; 32];
static VULKAN_NAME_INIT: std::sync::Once = std::sync::Once::new();

unsafe fn get_name_string_ptr() -> *const u8 {
    VULKAN_NAME_INIT.call_once(|| {
        let name = b"Vulkan (Reborn)";
        // Write SSO string
        ptr::copy_nonoverlapping(name.as_ptr(), VULKAN_NAME_STRING.as_mut_ptr(), 15);
        VULKAN_NAME_STRING[15] = 0; // null terminator
        // size = 15
        let size_ptr = VULKAN_NAME_STRING.as_mut_ptr().add(16) as *mut u64;
        *size_ptr = 15;
        // capacity = 15 (SSO marker)
        let cap_ptr = VULKAN_NAME_STRING.as_mut_ptr().add(24) as *mut u64;
        *cap_ptr = 15;
    });
    VULKAN_NAME_STRING.as_ptr()
}

/// C-ABI getName replacement — returns const String& (pointer to static string)
/// In MSVC x64, returning a reference is just returning a pointer in RAX
pub unsafe extern "C" fn custom_get_name(_this: *mut u8) -> *const u8 {
    get_name_string_ptr()
}

/// Information about a mapped vtable
pub struct VtableMap {
    pub d3d11_vtable: *const *const u8,
    pub cloned_vtable: *mut *const u8,
    pub vtable_size: usize,
    pub get_name_slot: Option<usize>,
}

impl VtableMap {
    /// Find the getName slot by examining which vtable entry returns a string
    /// matching "Direct3D11 Rendering Subsystem"
    pub unsafe fn find_get_name_slot(
        d3d11_rs: *mut u8,
        vtable: *const *const u8,
        max_slots: usize,
    ) -> Option<usize> {
        // getName returns a `const String&` which is a pointer to a std::string
        // The signature is: const String& getName() const
        // In x64, this is: const std::string* __fastcall getName(const RenderSystem* this)
        //
        // We look for the slot that, when called, returns a valid string pointer
        // containing "Direct3D"
        
        // The D3D11 name is stored somewhere in the object. Let's scan the object
        // for "Direct3D" first to know what the function should return.
        
        // Actually, for safety, we won't call arbitrary vtable entries.
        // Instead, we'll look at known offsets based on Ogre source.
        //
        // In Ogre 2.x, the virtual method order is typically:
        // [0] destructor
        // [1] getName
        // But MSVC may add RTTI entries...
        //
        // MSVC vtable layout for single inheritance:
        // [0] = first virtual function (often destructor if virtual dtor exists)
        //
        // For Ogre::RenderSystem (virtual destructor):
        // [0] = scalar deleting destructor  
        // [1] = getName() ← this is our target
        //
        // Let's try slot 1 first, then scan if that doesn't match.
        
        None // Will be filled after investigator run
    }
}
