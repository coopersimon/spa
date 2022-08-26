mod internal;
mod memory;
mod interrupt;
mod maths;
mod joypad;
mod ipc;
mod card;
mod rtc;
mod spi;
mod video;
mod input;

use arm::{
    ARM7TDMI, ARM9ES, ARMDriver, ARMCore
};
use crossbeam_channel::{Sender, Receiver, unbounded};

#[cfg(feature = "debug")]
use crate::common::debug::DebugInterface;
use crate::common::framecomms::{new_frame_comms, FrameRequester};
use internal::DS9InternalMem;
use memory::{
    DS9MemoryBus, DS7MemoryBus
};
use video::Renderer;
//use joypad::DSButtons;
use input::UserInput;
pub use memory::MemoryConfig;
pub use input::Button;

type RendererType = video::ProceduralRenderer;

pub struct NDS {
    frame_receiver: FrameRequester<UserInput>,
    current_input:  UserInput
}

impl NDS {
    pub fn new(config: MemoryConfig) -> Self {
        let (render_width, render_height) = RendererType::render_size();
        let (frame_sender, frame_receiver) = new_frame_comms(render_width * render_height * 4, 2);
        // The below is a bit dumb but it avoids sending the CPU (which introduces a ton of problems).
        // We have to extract the audio receivers from the CPU and get them in the main thread to use
        //   for the audio handler.
        //let (channel_sender, channel_receiver) = unbounded();
        let (mut arm9_bus, mut arm7_bus) = DS9MemoryBus::<RendererType>::new(&config, frame_sender);

        let fast_boot = config.fast_boot;
        let (fast_entry_arm9, fast_entry_arm7) = if fast_boot {
            let card_header = arm9_bus.get_header();
            arm9_bus.setup_boot_area(&card_header);
            arm7_bus.setup_boot_area(&card_header);
            (Some(card_header.arm9_entry_addr()), Some(card_header.arm7_entry_addr()))
        } else {
            (None, None)
        };

        std::thread::Builder::new().name("ARM9-CPU".to_string()).spawn(move || {
            let mut internal_mem = Box::new(DS9InternalMem::new(arm9_bus));
            if fast_boot {
                internal_mem.setup_init();
            }
            let mut cpu = new_arm9_cpu(internal_mem, fast_entry_arm9);
            loop {
                cpu.step();
            }
        }).unwrap();

        //let arm7_no_bios = config.ds7_bios_path.is_none();
        std::thread::Builder::new().name("ARM7-CPU".to_string()).spawn(move || {
            let mut cpu = new_arm7_cpu(arm7_bus, fast_entry_arm7, false);
            //let audio_channels = cpu.mut_mem().enable_audio();
            //channel_sender.send(audio_channels).unwrap();
            loop {
                cpu.step();
            }
        }).unwrap();

        //let audio_channels = channel_receiver.recv().unwrap();
        Self {
            frame_receiver: frame_receiver,
            //audio_channels: Some(audio_channels),
//
            current_input:  UserInput::default()
        }
    }

    /// Drives the emulator and returns a pair of frames.
    /// 
    /// This should be called at 60fps.
    /// The frames are in the format R8G8B8A8.
    pub fn frame(&mut self, upper_frame: &mut [u8], lower_frame: &mut [u8]) {
        self.frame_receiver.get_frame(&mut [upper_frame, lower_frame], self.current_input.clone());
    }

    pub fn render_size(&mut self) -> (usize, usize) {
        RendererType::render_size()
    }

    pub fn set_button(&mut self, button: Button, pressed: bool) {
        self.current_input.set_button(button, pressed);
    }

    /// Call with Some((x, y)) when the touchscreen is pressed.
    /// Coordinates should be between 0.0 and 1.0.
    /// 
    /// Call with None when the touchscreen is released.
    pub fn touchscreen_pressed(&mut self, coords: Option<(f64, f64)>) {
        self.current_input.set_touchscreen(coords);
    }
}

// Debug
#[cfg(feature = "debug")]
impl NDS {
    /// Make a new debuggable NDS.
    /// 
    /// Steps through the ARM7 CPU.
    pub fn new_debug_7(config: MemoryConfig) -> DebugInterface<UserInput> {
        use crate::common::framecomms::debug::new_debug_frame_comms;

        let (render_width, render_height) = RendererType::render_size();
        let (frame_sender, frame_receiver) = new_debug_frame_comms(render_width * render_height * 4, 2);
        let (debug_interface, debug_wrapper) = DebugInterface::new(frame_receiver, UserInput::default());

        let (mut arm9_bus, mut arm7_bus) = DS9MemoryBus::<RendererType>::new(&config, frame_sender);

        let fast_boot = config.fast_boot;
        let (fast_entry_arm9, fast_entry_arm7) = if fast_boot {
            let card_header = arm9_bus.get_header();
            arm9_bus.setup_boot_area(&card_header);
            arm7_bus.setup_boot_area(&card_header);
            (Some(card_header.arm9_entry_addr()), Some(card_header.arm7_entry_addr()))
        } else {
            (None, None)
        };

        std::thread::Builder::new().name("ARM9-CPU".to_string()).spawn(move || {
            let mut internal_mem = Box::new(DS9InternalMem::new(arm9_bus));
            if fast_boot {
                internal_mem.setup_init();
            }
            let mut cpu = new_arm9_cpu(internal_mem, fast_entry_arm9);
            loop {
                cpu.step();
            }
        }).unwrap();

        //let arm7_no_bios = config.ds7_bios_path.is_none();
        std::thread::Builder::new().name("ARM7-CPU".to_string()).spawn(move || {
            let cpu = new_arm7_cpu(arm7_bus, fast_entry_arm7, false);
            //let audio_channels = cpu.mut_mem().enable_audio();
            //channel_sender.send(audio_channels).unwrap();
            debug_wrapper.run_debug(cpu);
        }).unwrap();

        debug_interface
    }

    /// Make a new debuggable NDS.
    /// 
    /// Steps through the ARM9 CPU.
    pub fn new_debug_9(config: MemoryConfig) -> DebugInterface<UserInput> {
        use crate::common::framecomms::debug::new_debug_frame_comms;

        let (render_width, render_height) = RendererType::render_size();
        let (frame_sender, frame_receiver) = new_debug_frame_comms(render_width * render_height * 4, 2);
        let (debug_interface, debug_wrapper) = DebugInterface::new(frame_receiver, UserInput::default());

        let (mut arm9_bus, mut arm7_bus) = DS9MemoryBus::<RendererType>::new(&config, frame_sender);

        let fast_boot = config.fast_boot;
        let (fast_entry_arm9, fast_entry_arm7) = if fast_boot {
            let card_header = arm9_bus.get_header();
            arm9_bus.setup_boot_area(&card_header);
            arm7_bus.setup_boot_area(&card_header);
            (Some(card_header.arm9_entry_addr()), Some(card_header.arm7_entry_addr()))
        } else {
            (None, None)
        };


        std::thread::Builder::new().name("ARM9-CPU".to_string()).spawn(move || {
            let mut internal_mem = Box::new(DS9InternalMem::new(arm9_bus));
            if fast_boot {
                internal_mem.setup_init();
            }
            let cpu = new_arm9_cpu(internal_mem, fast_entry_arm9);
            debug_wrapper.run_debug(cpu);
        }).unwrap();

        std::thread::Builder::new().name("ARM7-CPU".to_string()).spawn(move || {
            let mut cpu = new_arm7_cpu(arm7_bus, fast_entry_arm7, false);
            //let audio_channels = cpu.mut_mem().enable_audio();
            //channel_sender.send(audio_channels).unwrap();
            loop {
                cpu.step();
            }
        }).unwrap();

        debug_interface
    }
}

fn new_arm7_cpu(mem_bus: Box<DS7MemoryBus>, fast_entry: Option<u32>, use_jit: bool) -> ARM7TDMI<DS7MemoryBus> {
    let mut cpu_builder = ARM7TDMI::new(mem_bus);
    if use_jit {
        cpu_builder = cpu_builder.enable_jit_in_ranges(vec![0..0x4000, 0x0800_0000..0x0E00_0000]);
    }
    if let Some(entry_point) = fast_entry {
        // Setup stack pointers.
        let mut cpu = cpu_builder.build();
        cpu.do_branch(entry_point);
        cpu.write_cpsr(arm::CPSR::SVC);
        cpu.write_reg(13, 0x0380_FFDC);
        cpu.write_cpsr(arm::CPSR::IRQ);
        cpu.write_reg(13, 0x0380_FFB0);
        cpu.write_cpsr(arm::CPSR::SYS);
        cpu.write_reg(13, 0x0380_FF00);
        cpu.write_cpsr(arm::CPSR::USR);
        cpu
    } else {
        cpu_builder.build()
    }
}

fn new_arm9_cpu<R: Renderer>(mem_bus: Box<DS9InternalMem<R>>, fast_entry: Option<u32>) -> ARM9ES<DS9InternalMem<R>> {
    let cpu_builder = ARM9ES::new(mem_bus);
    if let Some(entry_point) = fast_entry {
        // Setup stack pointers.
        let mut cpu = cpu_builder.build();
        cpu.do_branch(entry_point);
        cpu.write_cpsr(arm::CPSR::SVC);
        cpu.write_reg(13, 0x0080_3FC0);
        cpu.write_cpsr(arm::CPSR::IRQ);
        cpu.write_reg(13, 0x0080_3FA0);
        cpu.write_cpsr(arm::CPSR::SYS);
        cpu.write_reg(13, 0x0080_3EC0);
        cpu.write_cpsr(arm::CPSR::USR);
        cpu
    } else {
        cpu_builder.build()
    }
}