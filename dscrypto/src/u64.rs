#[inline]
pub const fn lo(val: u64) -> u32 {
    val as u32
}

#[inline]
pub const fn hi(val: u64) -> u32 {
    (val >> 32) as u32
}

#[inline]
pub const fn make(hi: u32, lo: u32) -> u64 {
    ((hi as u64) << 32) | (lo as u64)
}
