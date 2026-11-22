//! Target-specific math primitives with portable fallbacks.

/// Exact IEEE single-precision square root using the target's real FPU.
#[inline(always)]
pub fn sqrtf(value: f32) -> f32 {
    #[cfg(all(target_arch = "arm", feature = "arm-hardware-float"))]
    {
        let result: f32;
        // The explicit feature promises an ARM FPU. Daisy initializes CPACR
        // before audio processing; only compiler-allocated S registers change.
        unsafe {
            core::arch::asm!(
                "vsqrt.f32 {result}, {value}",
                result = lateout(sreg) result,
                value = in(sreg) value,
                options(nomem, nostack),
            );
        }
        result
    }
    #[cfg(not(all(target_arch = "arm", feature = "arm-hardware-float")))]
    libm::sqrtf(value)
}
