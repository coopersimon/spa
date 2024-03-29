/// Bit manipulation.

pub mod u8 {
    /// Set the nth bit.
    #[inline(always)]
    pub const fn bit(n: usize) -> u8 {
        1 << n
    }

    /// Set all bits between the top and bottom (inclusive).
    #[inline(always)]
    pub const fn bits(mut bottom: usize, top: usize) -> u8 {
        let mut out = 0;
        while bottom <= top {
            out |= bit(bottom);
            bottom += 1;
        }
        return out;
    }

    /// Check if the nth bit is set.
    #[inline(always)]
    pub const fn test_bit(val: u8, n: usize) -> bool {
        (val & bit(n)) != 0
    }
}

pub mod u16 {
    /// Set the nth bit.
    #[inline(always)]
    pub const fn bit(n: usize) -> u16 {
        1 << n
    }

    /// Set all bits between the top and bottom (inclusive).
    #[inline(always)]
    pub const fn bits(mut bottom: usize, top: usize) -> u16 {
        let mut out = 0;
        while bottom <= top {
            out |= bit(bottom);
            bottom += 1;
        }
        return out;
    }

    /// Check if the nth bit is set.
    #[inline(always)]
    pub const fn test_bit(val: u16, n: usize) -> bool {
        (val & bit(n)) != 0
    }
    
    /// Sign extend val from the number of bits specified.
    #[inline(always)]
    pub const fn sign_extend(val: u16, bits: usize) -> i16 {
        let shift = 16 - bits;
        let signed = val as i16;
        (signed << shift) >> shift
    }
}

pub mod u32 {
    /// Set the nth bit.
    #[inline(always)]
    pub const fn bit(n: usize) -> u32 {
        1 << n
    }

    /// Set all bits between the top and bottom (inclusive).
    #[inline(always)]
    pub const fn bits(mut bottom: usize, top: usize) -> u32 {
        let mut out = 0;
        while bottom <= top {
            out |= bit(bottom);
            bottom += 1;
        }
        return out;
    }

    /// Check if the nth bit is set.
    #[inline(always)]
    pub const fn test_bit(val: u32, n: usize) -> bool {
        (val & bit(n)) != 0
    }

    /// Sign extend val from the number of bits specified.
    #[inline(always)]
    pub const fn sign_extend(val: u32, bits: usize) -> i32 {
        let shift = 32 - bits;
        let signed = val as i32;
        (signed << shift) >> shift
    }
}