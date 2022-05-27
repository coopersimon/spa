
mod gba;
mod nds;

pub use gba::run_gba;
pub use nds::run_nds;

#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
struct Vertex {
    position:   [f32; 2],
    tex_coord:  [f32; 2]
}

unsafe impl bytemuck::Zeroable for Vertex {}
unsafe impl bytemuck::Pod for Vertex {}

fn pick_output_config(device: &cpal::Device) -> cpal::SupportedStreamConfigRange {
    use cpal::traits::DeviceTrait;

    const MIN: u32 = 32_000;

    let supported_configs_range = device.supported_output_configs()
        .expect("error while querying configs");

    for config in supported_configs_range {
        let cpal::SampleRate(v) = config.max_sample_rate();
        if v >= MIN {
            return config;
        }
    }

    device.supported_output_configs()
        .expect("error while querying formats")
        .next()
        .expect("No supported config")
}
