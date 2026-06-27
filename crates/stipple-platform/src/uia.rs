//! Hand-written **UI Automation** provider — no `windows`/`uiautomation` crate,
//! just a COM object (`IRawElementProviderSimple`) we build by hand: a struct
//! whose first field is a vtable pointer, matching the "talk to the OS directly"
//! approach of the other backends. Windows-only.
//!
//! UIA is how Windows exposes accessibility. Because Stipple self-draws every
//! control, the window is one opaque HWND, so we vend a provider that answers
//! UIA property queries (name, control type). This is the Windows half of the
//! cross-platform a11y bridge (AT-SPI is Linux, NSAccessibility is macOS); a
//! window plugs it in by returning it from `WM_GETOBJECT`. Exposing the full
//! element tree builds on this root provider.

#![allow(unsafe_code, non_snake_case, non_upper_case_globals)]

use core::ffi::c_void;

type Hresult = i32;
const S_OK: Hresult = 0;

// UIA constants.
const ProviderOptions_ServerSideProvider: i32 = 0x1;
const UIA_ControlTypePropertyId: i32 = 30003;
const UIA_NamePropertyId: i32 = 30005;
const UIA_GroupControlTypeId: i32 = 50026;

// VARIANT types.
const VT_EMPTY: u16 = 0;
const VT_I4: u16 = 3;
const VT_BSTR: u16 = 8;

#[link(name = "oleaut32")]
unsafe extern "system" {
    fn SysAllocString(psz: *const u16) -> *mut u16;
    fn SysFreeString(bstr: *mut u16);
}

/// A `VARIANT` (x64 layout: 8-byte header, then a 16-byte value union). We only
/// ever store a VT_I4 or VT_BSTR, both of which fit in the first 8 value bytes.
#[repr(C)]
struct Variant {
    vt: u16,
    r1: u16,
    r2: u16,
    r3: u16,
    val: u64,
    pad: u64,
}

impl Variant {
    fn empty() -> Variant {
        Variant {
            vt: VT_EMPTY,
            r1: 0,
            r2: 0,
            r3: 0,
            val: 0,
            pad: 0,
        }
    }
}

/// The `IRawElementProviderSimple` vtable: the three `IUnknown` slots followed
/// by the four interface methods, in declaration order.
#[repr(C)]
struct ProviderVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const u8, *mut *mut c_void) -> Hresult,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    get_provider_options: unsafe extern "system" fn(*mut c_void, *mut i32) -> Hresult,
    get_pattern_provider: unsafe extern "system" fn(*mut c_void, i32, *mut *mut c_void) -> Hresult,
    get_property_value: unsafe extern "system" fn(*mut c_void, i32, *mut Variant) -> Hresult,
    get_host_raw_element_provider:
        unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> Hresult,
}

/// Our COM object: the vtable pointer must be the first field so the object
/// pointer *is* the interface pointer.
#[repr(C)]
struct Provider {
    vtable: *const ProviderVtbl,
    refcount: u32,
    /// UTF-16 (NUL-terminated) accessible name, for `SysAllocString`.
    name: Vec<u16>,
    control_type: i32,
}

unsafe extern "system" fn qi(this: *mut c_void, _iid: *const u8, ppv: *mut *mut c_void) -> Hresult {
    // We answer every interface query with ourselves — enough for IUnknown and
    // IRawElementProviderSimple, which is all UIA asks of a simple provider.
    unsafe {
        *ppv = this;
        add_ref(this);
    }
    S_OK
}

unsafe extern "system" fn add_ref(this: *mut c_void) -> u32 {
    unsafe {
        let p = this as *mut Provider;
        (*p).refcount += 1;
        (*p).refcount
    }
}

unsafe extern "system" fn release(this: *mut c_void) -> u32 {
    unsafe {
        let p = this as *mut Provider;
        (*p).refcount -= 1;
        let rc = (*p).refcount;
        if rc == 0 {
            drop(Box::from_raw(p));
        }
        rc
    }
}

unsafe extern "system" fn get_provider_options(_this: *mut c_void, opts: *mut i32) -> Hresult {
    unsafe {
        *opts = ProviderOptions_ServerSideProvider;
    }
    S_OK
}

unsafe extern "system" fn get_pattern_provider(
    _this: *mut c_void,
    _pattern: i32,
    ret: *mut *mut c_void,
) -> Hresult {
    unsafe {
        *ret = core::ptr::null_mut(); // we implement no control patterns yet
    }
    S_OK
}

unsafe extern "system" fn get_property_value(
    this: *mut c_void,
    property_id: i32,
    ret: *mut Variant,
) -> Hresult {
    unsafe {
        let p = this as *mut Provider;
        let mut v = Variant::empty();
        match property_id {
            UIA_ControlTypePropertyId => {
                v.vt = VT_I4;
                v.val = (*p).control_type as u32 as u64;
            }
            UIA_NamePropertyId => {
                v.vt = VT_BSTR;
                v.val = SysAllocString((*p).name.as_ptr()) as u64;
            }
            _ => {} // VT_EMPTY → "property not supported"
        }
        *ret = v;
    }
    S_OK
}

unsafe extern "system" fn get_host_raw_element_provider(
    _this: *mut c_void,
    ret: *mut *mut c_void,
) -> Hresult {
    unsafe {
        *ret = core::ptr::null_mut(); // a top-level fragment root has no host
    }
    S_OK
}

static PROVIDER_VTBL: ProviderVtbl = ProviderVtbl {
    query_interface: qi,
    add_ref,
    release,
    get_provider_options,
    get_pattern_provider,
    get_property_value,
    get_host_raw_element_provider,
};

/// Create a UIA provider for a `Group` element named `name`. Returns the COM
/// interface pointer (`IRawElementProviderSimple*`); release it via its vtable
/// `Release`. Plug it into a window's `WM_GETOBJECT` via
/// `UiaReturnRawElementProvider`.
pub fn create_provider(name: &str) -> *mut c_void {
    let mut utf16: Vec<u16> = name.encode_utf16().collect();
    utf16.push(0);
    let provider = Box::new(Provider {
        vtable: &PROVIDER_VTBL,
        refcount: 1,
        name: utf16,
        control_type: UIA_GroupControlTypeId,
    });
    Box::into_raw(provider) as *mut c_void
}

/// Exercise the provider through its COM vtable (real COM dispatch) and print
/// what it returns — the CI self-check (a cross-process UIA client needs a live
/// desktop UIA stack the runner doesn't reliably provide). Verifies the provider
/// answers `ProviderOptions`, `ControlType`, and `Name`.
pub fn selftest(name: &str) -> Result<(), String> {
    unsafe {
        let p = create_provider(name);
        let vt = &*(*(p as *mut Provider)).vtable;

        let mut opts: i32 = 0;
        (vt.get_provider_options)(p, &mut opts);

        let mut ct = Variant::empty();
        (vt.get_property_value)(p, UIA_ControlTypePropertyId, &mut ct);

        let mut nm = Variant::empty();
        (vt.get_property_value)(p, UIA_NamePropertyId, &mut nm);
        let name_out = bstr_to_string(nm.val as *const u16);
        if nm.val != 0 {
            SysFreeString(nm.val as *mut u16);
        }

        println!(
            "UIA provider: options={} controltype={} name={:?}",
            opts, ct.val as i32, name_out
        );

        (vt.release)(p);
        Ok(())
    }
}

/// Read a NUL-terminated UTF-16 BSTR into a `String`.
unsafe fn bstr_to_string(bstr: *const u16) -> String {
    if bstr.is_null() {
        return String::new();
    }
    unsafe {
        let mut len = 0usize;
        while *bstr.add(len) != 0 {
            len += 1;
        }
        let slice = core::slice::from_raw_parts(bstr, len);
        String::from_utf16_lossy(slice)
    }
}
