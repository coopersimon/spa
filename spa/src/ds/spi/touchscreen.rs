
pub struct Touchscreen {

}

impl Touchscreen {
    pub fn new() -> Self {
        Self {

        }
    }

    pub fn deselect(&mut self) {
    }

    pub fn read(&mut self) -> u8 {
        //println!("read tsc");
        0
    }

    pub fn write(&mut self, data: u8) {
        //println!("write {:X} to tsc", data);
    }
}
