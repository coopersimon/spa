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

pub use memory::MemoryConfig;
pub use input::Button;

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

    /// Drives the emulator and returns a frame.
    /// 
    /// This should be called at 60fps.
    /// The frame is in the format R8G8B8A8.
    pub fn frame(&mut self, frame: &mut [u8]) {
        self.frame_receiver.get_frame(&mut [frame], self.buttons_pressed);
    }

    pub fn render_size(&mut self) -> (usize, usize) {
        RendererType::render_size()
    }

    /// Call this at the start to enable audio.
    /// It creates a GBAAudioHandler that can be sent to the audio thread.
    pub fn enable_audio(&mut self, sample_rate: f64) -> Option<GBAAudioHandler> {
        if let Some((sample_rx, rate_rx)) = self.audio_channels.take() {
            Some(GBAAudioHandler {
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

    pub fn set_button(&mut self, button: Button, pressed: bool) {
        self.buttons_pressed.set(button.into(), !pressed);
    }
}

/// Created by a GBA.
pub struct GBAAudioHandler {
    resampler:    Resampler,
}

impl GBAAudioHandler {
    /// Fill the provided buffer with samples.
    /// The format is PCM interleaved stereo.
    pub fn get_audio_packet(&mut self, buffer: &mut [f32]) {
        for (o_frame, i_frame) in buffer.chunks_exact_mut(2).zip(&mut self.resampler) {
            o_frame.copy_from_slice(&i_frame);
        }
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