/* automatically generated by rust-bindgen 0.70.1 */

pub type __u32 = ::std::os::raw::c_uint;
pub type __u64 = ::std::os::raw::c_ulonglong;
pub type u32_ = __u32;
pub type u64_ = __u64;
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct common_event {
    pub timestamp: u64_,
    pub smp_id: u32_,
}
