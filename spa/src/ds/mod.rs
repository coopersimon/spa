mod cache;
mod memory;
mod interrupt;
mod maths;
mod joypad;
mod ipc;
mod card;
mod rtc;
mod spi;
//mod video;

use arm::{
    ARM7TDMI, ARM9ES, ARMDriver, ARMCore
};
use crossbeam_channel::{Sender, Receiver, unbounded};

#[cfg(feature = "debug")]
use crate::common::debug::DebugInterface;
use cache::DS9InternalMem;
use memory::{
    DS9MemoryBus, DS7MemoryBus
};
use joypad::DSButtons;
pub use memory::MemoryConfig;

pub enum Button {
    A,
    B,
    X,
    Y,
    Start,
    Select,
    Left,
    Right,
    Up,
    Down,
    L,
    R
}

pub struct NDS {
    //buttons_pressed: Buttons,
}

impl NDS {
    pub fn new(config: MemoryConfig) -> Self {
        //let (render_width, render_height) = RendererType::render_size();
        //let (frame_sender, frame_receiver) = new_frame_comms(render_width * render_height * 4);
        // The below is a bit dumb but it avoids sending the CPU (which introduces a ton of problems).
        // We have to extract the audio receivers from the CPU and get them in the main thread to use
        //   for the audio handler.
        //let (channel_sender, channel_receiver) = unbounded();
        let (arm9_bus, arm7_bus) = DS9MemoryBus::new(&config);

        std::thread::Builder::new().name("ARM9-CPU".to_string()).spawn(move || {
            let internal_mem = Box::new(DS9InternalMem::new(arm9_bus));
            let mut cpu = new_arm9_cpu(internal_mem);
            loop {
                cpu.step();
            }
        }).unwrap();

        let arm7_no_bios = config.ds7_bios_path.is_none();
        std::thread::Builder::new().name("ARM7-CPU".to_string()).spawn(move || {
            let mut cpu = new_arm7_cpu(arm7_bus, arm7_no_bios, false);
            //let audio_channels = cpu.mut_mem().enable_audio();
            //channel_sender.send(audio_channels).unwrap();
            loop {
                cpu.step();
            }
        }).unwrap();

        //let audio_channels = channel_receiver.recv().unwrap();
        Self {
            //frame_receiver: frame_receiver,
            //audio_channels: Some(audio_channels),
//
            //buttons_pressed: Buttons::from_bits_truncate(0xFFFF),
        }
    }

    /// Drives the emulator and returns a frame.
    /// 
    /// This should be called at 60fps.
    /// The frame is in the format R8G8B8A8.
    pub fn frame(&mut self, frame: &mut [u8]) {
        //self.frame_receiver.get_frame(frame, self.buttons_pressed);
    }

    pub fn set_button(&mut self, button: Button, pressed: bool) {
        //self.buttons_pressed.set(button.into(), !pressed);
    }
}

// Debug
#[cfg(feature = "debug")]
impl NDS {
    /// Make a new debuggable NDS.
    /// 
    /// Steps through the ARM7 CPU.
    pub fn new_debug_7(config: MemoryConfig) -> DebugInterface<DSButtons> {
        use crate::common::framecomms::debug::new_debug_frame_comms;

        let (render_width, render_height) = (256, 384);//RendererType::render_size();
        let (frame_sender, frame_receiver) = new_debug_frame_comms(render_width * render_height * 4);
        let (debug_interface, debug_wrapper) = DebugInterface::new(frame_receiver, DSButtons::default());

        let (arm9_bus, arm7_bus) = DS9MemoryBus::new(&config);

        std::thread::Builder::new().name("ARM9-CPU".to_string()).spawn(move || {
            let internal_mem = Box::new(DS9InternalMem::new(arm9_bus));
            let mut cpu = new_arm9_cpu(internal_mem);
            loop {
                cpu.step();
            }
        }).unwrap();

        let arm7_no_bios = config.ds7_bios_path.is_none();
        std::thread::Builder::new().name("ARM7-CPU".to_string()).spawn(move || {
            let mut cpu = new_arm7_cpu(arm7_bus, arm7_no_bios, false);
            //let audio_channels = cpu.mut_mem().enable_audio();
            //channel_sender.send(audio_channels).unwrap();
            debug_wrapper.run_debug(cpu);
        }).unwrap();

        debug_interface
    }

    /// Make a new debuggable NDS.
    /// 
    /// Steps through the ARM9 CPU.
    pub fn new_debug_9(config: MemoryConfig) -> DebugInterface<DSButtons> {
        use crate::common::framecomms::debug::new_debug_frame_comms;

        let (render_width, render_height) = (256, 384);//RendererType::render_size();
        let (frame_sender, frame_receiver) = new_debug_frame_comms(render_width * render_height * 4);
        let (debug_interface, debug_wrapper) = DebugInterface::new(frame_receiver, DSButtons::default());

        let (arm9_bus, arm7_bus) = DS9MemoryBus::new(&config);

        std::thread::Builder::new().name("ARM9-CPU".to_string()).spawn(move || {
            let internal_mem = Box::new(DS9InternalMem::new(arm9_bus));
            let mut cpu = new_arm9_cpu(internal_mem);
            debug_wrapper.run_debug(cpu);
        }).unwrap();

        let arm7_no_bios = config.ds7_bios_path.is_none();
        std::thread::Builder::new().name("ARM7-CPU".to_string()).spawn(move || {
            let mut cpu = new_arm7_cpu(arm7_bus, arm7_no_bios, false);
            //let audio_channels = cpu.mut_mem().enable_audio();
            //channel_sender.send(audio_channels).unwrap();
            loop {
                cpu.step();
            }
        }).unwrap();

        debug_interface
    }
}

fn new_arm7_cpu(mem_bus: Box<DS7MemoryBus>, no_bios: bool, use_jit: bool) -> ARM7TDMI<DS7MemoryBus> {
    let mut cpu_builder = ARM7TDMI::new(mem_bus);
    if use_jit {
        cpu_builder = cpu_builder.enable_jit_in_ranges(vec![0..0x4000, 0x0800_0000..0x0E00_0000]);
    }
    if no_bios {
        // Setup stack pointers.
        let mut cpu = cpu_builder.build();//set_swi_hook(emulated_swi).build();
        cpu.do_branch(0x0800_0000);
        cpu.write_cpsr(arm::CPSR::SVC);
        cpu.write_reg(13, 0x0300_7FE0);
        cpu.write_cpsr(arm::CPSR::IRQ);
        cpu.write_reg(13, 0x0300_7FA0);
        cpu.write_cpsr(arm::CPSR::SYS);
        cpu.write_reg(13, 0x0300_7F00);
        cpu.write_cpsr(arm::CPSR::USR);
        cpu
    } else {
        cpu_builder.build()
    }
}

fn new_arm9_cpu(mem_bus: Box<DS9InternalMem>) -> ARM9ES<DS9InternalMem> {
    let mut cpu_builder = ARM9ES::new(mem_bus);
    cpu_builder.build()
}