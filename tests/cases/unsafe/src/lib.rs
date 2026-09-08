// EXPECT: Unsafe Without Comment
// EXPECT: Transmute Usage
// EXPECT: Raw Pointer Arithmetic
// EXPECT: Multi Mut Ref Unsafe
// EXPECT: FFI Without Wrapper
// EXPECT: Inline Assembly
// EXPECT: Unsafe Fn Missing Safety Docs
// EXPECT: Unsafe Impl Missing Safety Docs
// EXPECT: Large Unsafe Block
// EXPECT: FFI Type Not repr(C)

#![allow(dead_code, improper_ctypes, unused_unsafe)]

pub struct FfiHeader {
    len: u32,
}

unsafe extern "C" {
    fn c_open(value: i32) -> i32;
}

pub unsafe fn from_raw(ptr: *const u8) -> u8 {
    unsafe { ptr.read() }
}

struct Sendable(*mut u8);
unsafe impl Send for Sendable {}

// The raw API is only called from unsafe code, so local safe wrapper coverage
// remains an exploratory review candidate.
unsafe fn unsafe_patterns(ptr: *mut i32, value: u32) {
    unsafe {
        let _cast: f32 = std::mem::transmute(value);
        let _ = ptr.add(1).read();
        let a = &mut *ptr;
        let b = &mut *ptr;
        *a += *b;
        let _ = c_open(1);
        core::arch::asm!("nop");
        let mut total = 0;
        total += 1;
        total += 2;
        total += 3;
        total += 4;
        total += 5;
        total += 6;
        total += 7;
        total += 8;
        total += 9;
        total += 10;
        let _ = total;
    }
}
