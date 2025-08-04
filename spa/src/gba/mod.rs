mod memory;
mod interrupt;
mod video;
mod audio;
mod input;

use arm::{
    ARM7TDMI, ARMDriver, ARMCore
};
use crossbeam_channel::{Receiver, unbounded};

use crate::common::{
    video::framecomms::{new_frame_comms, FrameRequester},
    peripheral::joypad::Buttons,
    resampler::{Resampler, SamplePacket}
};
#[cfg(feature = "debug")]
use crate::common::debug::DebugInterface;
use memory::{
    MemoryBus,
    emulated_swi
};
use video::Renderer;
use audio::REAL_BASE_SAMPLE_RATE;
use super::{
    AudioHandler, Device, Button, Coords
};

pub use memory::MemoryConfig;

type RendererType = video::ProceduralRenderer;

pub struct GBA {
    frame_receiver: FrameRequester<Buttons>,
    audio_channels: Option<(Receiver<SamplePacket>, Receiver<f64>)>,

    buttons_pressed: Buttons,
}

impl GBA {
    pub fn new(config: MemoryConfig) -> Self {
        let (render_width, render_height) = RendererType::render_size();
        let (frame_sender, frame_receiver) = new_frame_comms(render_width * render_height * 4, 1);
        // The below is a bit dumb but it avoids sending the CPU (which introduces a ton of problems).
        // We have to extract the audio receivers from the CPU and get them in the main thread to use
        //   for the audio handler.
        let (channel_sender, channel_receiver) = unbounded();
        std::thread::Builder::new().name("CPU".to_string()).spawn(move || {
            let no_bios = config.bios_path.is_none();
            let bus = MemoryBus::<RendererType>::new(&config, frame_sender).unwrap();
            let mut cpu = new_cpu(bus, no_bios, false);
            let audio_channels = cpu.mut_mem().enable_audio();
            channel_sender.send(audio_channels).unwrap();
            loop {
                cpu.step();
            }
        }).unwrap();
        let audio_channels = channel_receiver.recv().unwrap();
        Self {
            frame_receiver: frame_receiver,
            audio_channels: Some(audio_channels),

            buttons_pressed: Buttons::from_bits_truncate(0xFFFF),
        }
    }
}

impl Device for GBA {
    fn frame(&mut self, upper_frame: &mut [u8], _lower_frame: &mut [u8]) {
        self.frame_receiver.get_frame(&mut [upper_frame], self.buttons_pressed);
    }

    fn render_size(&self) -> [Coords<usize>; 2] {
        let render_size = RendererType::render_size();
        [Coords {x: render_size.0, y: render_size.1}, Coords {x: 0, y: 0}]
    }

    fn enable_audio(&mut self, sample_rate: f64) -> Option<AudioHandler> {
        if let Some((sample_rx, rate_rx)) = self.audio_channels.take() {
            Some(AudioHandler {
                resampler: Resampler::new(
                    sample_rx,
                    Some(rate_rx),
                    REAL_BASE_SAMPLE_RATE,
                    sample_rate
                ),
            })
        } else {
            None
        }
    }

    fn set_button(&mut self, button: Button, pressed: bool) {
        self.buttons_pressed.set(button.into(), !pressed);
    }

    fn touchscreen_pressed(&mut self, _coords: Option<Coords<f64>>) {
        // No effect on GBA.
    }
}

// Debug
#[cfg(feature = "debug")]
impl GBA {
    /// Make a new debuggable GBA.
    pub fn new_debug(config: MemoryConfig) -> DebugInterface<Buttons> {
        use crate::common::video::framecomms::debug::new_debug_frame_comms;

        let (render_width, render_height) = RendererType::render_size();
        let (frame_sender, frame_receiver) = new_debug_frame_comms(render_width * render_height * 4, 1);
        let (debug_interface, debug_wrapper) = DebugInterface::new(frame_receiver, Buttons::from_bits_truncate(0xFFFF));

        std::thread::Builder::new().name("CPU".to_string()).spawn(move || {
            let no_bios = config.bios_path.is_none();
            let bus = MemoryBus::<RendererType>::new(&config, frame_sender).unwrap();
            let cpu = new_cpu(bus, no_bios, false);
            debug_wrapper.run_debug(cpu);
        }).unwrap();

        debug_interface
    }
}

fn new_cpu(mem_bus: Box<MemoryBus<RendererType>>, no_bios: bool, use_jit: bool) -> ARM7TDMI<MemoryBus<RendererType>> {
    let mut cpu_builder = ARM7TDMI::new(mem_bus);
    if use_jit {
        cpu_builder = cpu_builder.enable_jit_in_ranges(vec![0..0x4000, 0x0800_0000..0x0E00_0000]);
    }
    if no_bios {
        // Setup stack pointers.
        let mut cpu = cpu_builder.set_swi_hook(emulated_swi).build();
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