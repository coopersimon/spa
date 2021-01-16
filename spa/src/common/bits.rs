/// Bit manipulation.

pub mod u8 {
    /// Set the nth bit.
    pub const fn bit(n: usize) -> u8 {
        1 << n
    }

    /// Set all bits between the top and bottom (inclusive).
    pub const fn bits(mut bottom: usize, top: usize) -> u8 {
        let mut out = 0;
        while bottom <= top {
            out |= bit(bottom);
            bottom += 1;
        }
        return out;
    }

    /// Check if the nth bit is set.
    pub const fn test_bit(val: u8, n: usize) -> bool {
        (val & bit(n)) != 0
    }
}

pub mod u16 {
    /// Set the nth bit.
    pub const fn bit(n: usize) -> u16 {
        1 << n
    }

    /// Set all bits between the top and bottom (inclusive).
    pub const fn bits(mut bottom: usize, top: usize) -> u16 {
        let mut out = 0;
        while bottom <= top {
            out |= bit(bottom);
            bottom += 1;
        }
        return out;
    }

    /// Check if the nth bit is set.
    pub const fn test_bit(val: u16, n: usize) -> bool {
        (val & bit(n)) != 0
    }
}