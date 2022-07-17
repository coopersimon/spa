use fixed::types::I20F12;

pub type N = I20F12;

#[derive(Clone, Default)]
pub struct Matrix {
    pub elements: [N; 16]
}

impl Matrix {
    pub fn identity() -> Self {
        Self {
            elements: [
                N::ONE, N::ZERO, N::ZERO, N::ZERO,
                N::ZERO, N::ONE, N::ZERO, N::ZERO,
                N::ZERO, N::ZERO, N::ONE, N::ZERO,
                N::ZERO, N::ZERO, N::ZERO, N::ONE
            ]
        }
    }

    pub fn from_4x4(elements: &[N]) -> Self {
        Self {
            elements: [
                elements[0], elements[1], elements[2], elements[3],
                elements[4], elements[5], elements[6], elements[7],
                elements[8], elements[9], elements[10], elements[11],
                elements[12], elements[13], elements[14], elements[15],
            ]
        }
    }

    pub fn from_4x3(elements: &[N]) -> Self {
        Self {
            elements: [
                elements[0], elements[1], elements[2], N::ZERO,
                elements[3], elements[4], elements[5], N::ZERO,
                elements[6], elements[7], elements[8], N::ZERO,
                elements[9], elements[10], elements[11], N::ONE,
            ]
        }
    }
    
    /// Multiply other by the matrix, mutating the matrix.
    /// 
    /// Current = C, new current = C', other = M
    /// 
    /// C' = MC
    pub fn mul_4x4(&mut self, other: &[N]) {
        let m00 = other[0] * self.elements[0] + other[1] * self.elements[4] + other[2] * self.elements[8] + other[3] * self.elements[12];
        let m10 = other[0] * self.elements[1] + other[1] * self.elements[5] + other[2] * self.elements[9] + other[3] * self.elements[13];
        let m20 = other[0] * self.elements[2] + other[1] * self.elements[6] + other[2] * self.elements[10] + other[3] * self.elements[14];
        let m30 = other[0] * self.elements[3] + other[1] * self.elements[7] + other[2] * self.elements[11] + other[3] * self.elements[15];
        
        let m01 = other[4] * self.elements[0] + other[5] * self.elements[4] + other[6] * self.elements[8] + other[7] * self.elements[12];
        let m11 = other[4] * self.elements[1] + other[5] * self.elements[5] + other[6] * self.elements[9] + other[7] * self.elements[13];
        let m21 = other[4] * self.elements[2] + other[5] * self.elements[6] + other[6] * self.elements[10] + other[7] * self.elements[14];
        let m31 = other[4] * self.elements[3] + other[5] * self.elements[7] + other[6] * self.elements[11] + other[7] * self.elements[15];
        
        let m02 = other[8] * self.elements[0] + other[9] * self.elements[4] + other[10] * self.elements[8] + other[11] * self.elements[12];
        let m12 = other[8] * self.elements[1] + other[9] * self.elements[5] + other[10] * self.elements[9] + other[11] * self.elements[13];
        let m22 = other[8] * self.elements[2] + other[9] * self.elements[6] + other[10] * self.elements[10] + other[11] * self.elements[14];
        let m32 = other[8] * self.elements[3] + other[9] * self.elements[7] + other[10] * self.elements[11] + other[11] * self.elements[15];
        
        let m03 = other[12] * self.elements[0] + other[13] * self.elements[4] + other[14] * self.elements[8] + other[15] * self.elements[12];
        let m13 = other[12] * self.elements[1] + other[13] * self.elements[5] + other[14] * self.elements[9] + other[15] * self.elements[13];
        let m23 = other[12] * self.elements[2] + other[13] * self.elements[6] + other[14] * self.elements[10] + other[15] * self.elements[14];
        let m33 = other[12] * self.elements[3] + other[13] * self.elements[7] + other[14] * self.elements[11] + other[15] * self.elements[15];

        self.elements[0] = m00;
        self.elements[1] = m10;
        self.elements[2] = m20;
        self.elements[3] = m30;
        self.elements[4] = m01;
        self.elements[5] = m11;
        self.elements[6] = m21;
        self.elements[7] = m31;
        self.elements[8] = m02;
        self.elements[9] = m12;
        self.elements[10] = m22;
        self.elements[11] = m32;
        self.elements[12] = m03;
        self.elements[13] = m13;
        self.elements[14] = m23;
        self.elements[15] = m33;
    }
    
    /// Multiply other by the matrix, mutating the matrix.
    /// 
    /// Current = C, new current = C', other = M
    /// 
    /// C' = MC
    pub fn mul_4x3(&mut self, other: &[N]) {
        let m00 = other[0] * self.elements[0] + other[1] * self.elements[4] + other[2] * self.elements[8];
        let m10 = other[0] * self.elements[1] + other[1] * self.elements[5] + other[2] * self.elements[9];
        let m20 = other[0] * self.elements[2] + other[1] * self.elements[6] + other[2] * self.elements[10];
        let m30 = other[0] * self.elements[3] + other[1] * self.elements[7] + other[2] * self.elements[11];
        
        let m01 = other[3] * self.elements[0] + other[4] * self.elements[4] + other[5] * self.elements[8];
        let m11 = other[3] * self.elements[1] + other[4] * self.elements[5] + other[5] * self.elements[9];
        let m21 = other[3] * self.elements[2] + other[4] * self.elements[6] + other[5] * self.elements[10];
        let m31 = other[3] * self.elements[3] + other[4] * self.elements[7] + other[5] * self.elements[11];
        
        let m02 = other[6] * self.elements[0] + other[7] * self.elements[4] + other[8] * self.elements[8];
        let m12 = other[6] * self.elements[1] + other[7] * self.elements[5] + other[8] * self.elements[9];
        let m22 = other[6] * self.elements[2] + other[7] * self.elements[6] + other[8] * self.elements[10];
        let m32 = other[6] * self.elements[3] + other[7] * self.elements[7] + other[8] * self.elements[11];
        
        let m03 = other[9] * self.elements[0] + other[10] * self.elements[4] + other[11] * self.elements[8] + self.elements[12];
        let m13 = other[9] * self.elements[1] + other[10] * self.elements[5] + other[11] * self.elements[9] + self.elements[13];
        let m23 = other[9] * self.elements[2] + other[10] * self.elements[6] + other[11] * self.elements[10] + self.elements[14];
        let m33 = other[9] * self.elements[3] + other[10] * self.elements[7] + other[11] * self.elements[11] + self.elements[15];

        self.elements[0] = m00;
        self.elements[1] = m10;
        self.elements[2] = m20;
        self.elements[3] = m30;
        self.elements[4] = m01;
        self.elements[5] = m11;
        self.elements[6] = m21;
        self.elements[7] = m31;
        self.elements[8] = m02;
        self.elements[9] = m12;
        self.elements[10] = m22;
        self.elements[11] = m32;
        self.elements[12] = m03;
        self.elements[13] = m13;
        self.elements[14] = m23;
        self.elements[15] = m33;
    }

    /// Multiply other by the matrix, mutating the matrix.
    /// 
    /// Current = C, new current = C', other = M
    /// 
    /// C' = MC
    pub fn mul_3x3(&mut self, other: &[N]) {
        let m00 = other[0] * self.elements[0] + other[1] * self.elements[4] + other[2] * self.elements[8];
        let m10 = other[0] * self.elements[1] + other[1] * self.elements[5] + other[2] * self.elements[9];
        let m20 = other[0] * self.elements[2] + other[1] * self.elements[6] + other[2] * self.elements[10];
        let m30 = other[0] * self.elements[3] + other[1] * self.elements[7] + other[2] * self.elements[11];
        
        let m01 = other[3] * self.elements[0] + other[4] * self.elements[4] + other[5] * self.elements[8];
        let m11 = other[3] * self.elements[1] + other[4] * self.elements[5] + other[5] * self.elements[9];
        let m21 = other[3] * self.elements[2] + other[4] * self.elements[6] + other[5] * self.elements[10];
        let m31 = other[3] * self.elements[3] + other[4] * self.elements[7] + other[5] * self.elements[11];
        
        let m02 = other[6] * self.elements[0] + other[7] * self.elements[4] + other[8] * self.elements[8];
        let m12 = other[6] * self.elements[1] + other[7] * self.elements[5] + other[8] * self.elements[9];
        let m22 = other[6] * self.elements[2] + other[7] * self.elements[6] + other[8] * self.elements[10];
        let m32 = other[6] * self.elements[3] + other[7] * self.elements[7] + other[8] * self.elements[11];

        self.elements[0] = m00;
        self.elements[1] = m10;
        self.elements[2] = m20;
        self.elements[3] = m30;
        self.elements[4] = m01;
        self.elements[5] = m11;
        self.elements[6] = m21;
        self.elements[7] = m31;
        self.elements[8] = m02;
        self.elements[9] = m12;
        self.elements[10] = m22;
        self.elements[11] = m32;
    }
    
    /// Multiply other by the matrix, mutating the matrix.
    /// 
    /// Current = C, new current = C', other = M
    /// 
    /// C' = MC
    pub fn mul_trans(&mut self, other: &[N]) {
        self.elements[12] += other[0] * self.elements[0] + other[1] * self.elements[4] + other[2] * self.elements[8];
        self.elements[13] += other[0] * self.elements[1] + other[1] * self.elements[5] + other[2] * self.elements[9];
        self.elements[14] += other[0] * self.elements[2] + other[1] * self.elements[6] + other[2] * self.elements[10];
        self.elements[15] += other[0] * self.elements[3] + other[1] * self.elements[7] + other[2] * self.elements[11];
    }
    
    /// Multiply other by the matrix, mutating the matrix.
    /// 
    /// Current = C, new current = C', other = M
    /// 
    /// C' = MC
    pub fn mul_scale(&mut self, other: &[N]) {
        self.elements[0] *= other[0];
        self.elements[1] *= other[0];
        self.elements[2] *= other[0];
        self.elements[3] *= other[0];
        
        self.elements[4] *= other[1];
        self.elements[5] *= other[1];
        self.elements[6] *= other[1];
        self.elements[7] *= other[1];
        
        self.elements[8] *= other[2];
        self.elements[9] *= other[2];
        self.elements[10] *= other[2];
        self.elements[11] *= other[2];
    }

    /*pub fn from_3x3(elements: &[N]) -> Self {
        Self {
            elements: [
                elements[0], elements[1], elements[2], N::ZERO,
                elements[3], elements[4], elements[5], N::ZERO,
                elements[6], elements[7], elements[8], N::ZERO,
                N::ZERO, N::ZERO, N::ZERO, N::ONE
            ]
        }
    }
    
    pub fn translation(elements: &[N]) -> Self {
        Self {
            elements: [
                N::ONE, N::ZERO, N::ZERO, N::ZERO,
                N::ZERO, N::ONE, N::ZERO, N::ZERO,
                N::ZERO, N::ZERO, N::ONE, N::ZERO,
                elements[0], elements[1], elements[2], N::ONE,
            ]
        }
    }
    
    pub fn scale(elements: &[N]) -> Self {
        Self {
            elements: [
                elements[0], N::ZERO, N::ZERO, N::ZERO,
                N::ZERO, elements[1], N::ZERO, N::ZERO,
                N::ZERO, N::ZERO, elements[2], N::ZERO,
                N::ZERO, N::ZERO, N::ZERO, N::ONE
            ]
        }
    }*/

    pub fn mul(&self, other: &Self) -> Self {
        Self {
            elements: [
                self.elements[0] * other.elements[0] + self.elements[1] * other.elements[4] + self.elements[2] * other.elements[8] + self.elements[3] * other.elements[12],
                self.elements[0] * other.elements[1] + self.elements[1] * other.elements[5] + self.elements[2] * other.elements[9] + self.elements[3] * other.elements[13],
                self.elements[0] * other.elements[2] + self.elements[1] * other.elements[6] + self.elements[2] * other.elements[10] + self.elements[3] * other.elements[14],
                self.elements[0] * other.elements[3] + self.elements[1] * other.elements[7] + self.elements[2] * other.elements[11] + self.elements[3] * other.elements[15],
                
                self.elements[4] * other.elements[0] + self.elements[5] * other.elements[4] + self.elements[6] * other.elements[8] + self.elements[7] * other.elements[12],
                self.elements[4] * other.elements[1] + self.elements[5] * other.elements[5] + self.elements[6] * other.elements[9] + self.elements[7] * other.elements[13],
                self.elements[4] * other.elements[2] + self.elements[5] * other.elements[6] + self.elements[6] * other.elements[10] + self.elements[7] * other.elements[14],
                self.elements[4] * other.elements[3] + self.elements[5] * other.elements[7] + self.elements[6] * other.elements[11] + self.elements[7] * other.elements[15],
                
                self.elements[8] * other.elements[0] + self.elements[9] * other.elements[4] + self.elements[10] * other.elements[8] + self.elements[11] * other.elements[12],
                self.elements[8] * other.elements[1] + self.elements[9] * other.elements[5] + self.elements[10] * other.elements[9] + self.elements[11] * other.elements[13],
                self.elements[8] * other.elements[2] + self.elements[9] * other.elements[6] + self.elements[10] * other.elements[10] + self.elements[11] * other.elements[14],
                self.elements[8] * other.elements[3] + self.elements[9] * other.elements[7] + self.elements[10] * other.elements[11] + self.elements[11] * other.elements[15],
                
                self.elements[12] * other.elements[0] + self.elements[13] * other.elements[4] + self.elements[14] * other.elements[8] + self.elements[15] * other.elements[12],
                self.elements[12] * other.elements[1] + self.elements[13] * other.elements[5] + self.elements[14] * other.elements[9] + self.elements[15] * other.elements[13],
                self.elements[12] * other.elements[2] + self.elements[13] * other.elements[6] + self.elements[14] * other.elements[10] + self.elements[15] * other.elements[14],
                self.elements[12] * other.elements[3] + self.elements[13] * other.elements[7] + self.elements[14] * other.elements[11] + self.elements[15] * other.elements[15],
            ]
        }
    }
}
