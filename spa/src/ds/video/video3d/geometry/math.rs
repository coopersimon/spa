use fixed::types::I20F12;

pub type N = I20F12;

#[derive(Clone)]
pub struct Vector<const S: usize> {
    pub elements: [N; S]
}

impl<const S: usize> Default for Vector<S> {
    fn default() -> Self {
        Self { elements: [N::ZERO; S] }
    }
}

impl<const S: usize> Vector<S> {
    pub fn new(from_elements: [N; S]) -> Self {
        Self {
            elements: from_elements
        }
    }

    #[inline]
    pub fn x(&self) -> N {
        self.elements[0]
    }
    
    #[inline]
    pub fn y(&self) -> N {
        self.elements[1]
    }
    
    #[inline]
    pub fn z(&self) -> N {
        self.elements[2]
    }
    
    #[inline]
    pub fn w(&self) -> N {
        self.elements[3]
    }

    pub fn dot_product(&self, other: &Self) -> N {
        self.elements.iter().zip(&other.elements).fold(N::ZERO, |acc, (a, b)| acc + (a * b))
        //self.elements[0] * other.elements[0] + self.elements[1] * other.elements[1] + self.elements[2] * other.elements[2]
    }
}

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
    /// 
    /// The rows of other are multiplied by the columns of self.
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

    /// Multiply a 3-dimensional vector by the matrix.
    /// 
    /// Used for normal & lighting calculations.
    pub fn mul_vector_3(&self, vector: &Vector<3>) -> Vector<3> {
        Vector::new([
            vector.elements[0] * self.elements[0] + vector.elements[1] * self.elements[4] + vector.elements[2] * self.elements[8],
            vector.elements[0] * self.elements[1] + vector.elements[1] * self.elements[5] + vector.elements[2] * self.elements[9],
            vector.elements[0] * self.elements[2] + vector.elements[1] * self.elements[6] + vector.elements[2] * self.elements[10],
        ])
    }
    
    /// Multiply a 4-dimensional vector by the matrix.
    /// 
    /// Used for vertex calculations.
    pub fn mul_vector_4(&self, vector: &Vector<4>) -> Vector<4> {
        // TODO: input could implicitly use W=1?
        Vector::new([
            vector.elements[0] * self.elements[0] + vector.elements[1] * self.elements[4] + vector.elements[2] * self.elements[8] + vector.elements[3] * self.elements[12],
            vector.elements[0] * self.elements[1] + vector.elements[1] * self.elements[5] + vector.elements[2] * self.elements[9] + vector.elements[3] * self.elements[13],
            vector.elements[0] * self.elements[2] + vector.elements[1] * self.elements[6] + vector.elements[2] * self.elements[10] + vector.elements[3] * self.elements[14],
            vector.elements[0] * self.elements[3] + vector.elements[1] * self.elements[7] + vector.elements[2] * self.elements[11] + vector.elements[3] * self.elements[15],
        ])
    }
}

impl std::fmt::Display for Matrix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{:<10X}{:<10X}{:<10X}{:<10X}", self.elements[0], self.elements[1], self.elements[2], self.elements[3])?;
        writeln!(f, "{:<10X}{:<10X}{:<10X}{:<10X}", self.elements[4], self.elements[5], self.elements[6], self.elements[7])?;
        writeln!(f, "{:<10X}{:<10X}{:<10X}{:<10X}", self.elements[8], self.elements[9], self.elements[10], self.elements[11])?;
        writeln!(f, "{:<10X}{:<10X}{:<10X}{:<10X}", self.elements[12], self.elements[13], self.elements[14], self.elements[15])
    }
}