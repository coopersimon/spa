use spa::gba;

use winit::{
    dpi::{
        Size, LogicalSize
    },
    event::{
        Event, WindowEvent,
        ElementState,
        VirtualKeyCode,
    },
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder
};
use cpal::traits::StreamTrait;
use super::Vertex;

pub fn run_gba(config: gba::MemoryConfig, mute: bool) {

    let mut gba = gba::GBA::new(config);
    let render_size = gba.render_size();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size(Size::Logical(LogicalSize{width: (render_size.0 * 2) as f64, height: (render_size.1 * 2) as f64}))
        .with_title("SPA")
        .build(&event_loop).unwrap();

    // Setup wgpu
    let surface = wgpu::Surface::create(&window);

    let adapter = futures::executor::block_on(wgpu::Adapter::request(
        &wgpu::RequestAdapterOptions {
            power_preference:   wgpu::PowerPreference::Default,
            compatible_surface: Some(&surface)
        },
        wgpu::BackendBit::PRIMARY
    )).unwrap();

    let (device, queue) = futures::executor::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        extensions: wgpu::Extensions {
            anisotropic_filtering: false,
        },
        limits: wgpu::Limits::default()
    }));

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        bindings: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::SampledTexture {
                    multisampled: false,
                    component_type: wgpu::TextureComponentType::Uint,
                    dimension: wgpu::TextureViewDimension::D2,
                },
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStage::FRAGMENT,
                ty: wgpu::BindingType::Sampler { comparison: false },
            },
        ],
        label: None
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        bind_group_layouts: &[&bind_group_layout],
    });

    let texture_extent = wgpu::Extent3d {
        width: render_size.0 as u32,
        height: render_size.1 as u32,
        depth: 1
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        size: texture_extent,
        array_layer_count: 1,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
        label: None,
    });
    let texture_view = texture.create_default_view();

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter:     wgpu::FilterMode::Nearest,
        min_filter:     wgpu::FilterMode::Linear,
        mipmap_filter:  wgpu::FilterMode::Nearest,
        lod_min_clamp:  0.0,
        lod_max_clamp:  100.0,
        compare:        wgpu::CompareFunction::Undefined,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &bind_group_layout,
        bindings: &[
            wgpu::Binding {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&texture_view)
            },
            wgpu::Binding {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler)
            }
        ],
        label: None
    });

    let size = window.inner_size();

    let mut swapchain_desc = wgpu::SwapChainDescriptor {
        usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::Fifo
    };

    let mut swapchain = device.create_swap_chain(&surface, &swapchain_desc);

    let vertices = vec![
        Vertex{position: [-1.0, -1.0], tex_coord: [0.0, 1.0]},
        Vertex{position: [1.0, -1.0], tex_coord: [1.0, 1.0]},
        Vertex{position: [-1.0, 1.0], tex_coord: [0.0, 0.0]},
        Vertex{position: [1.0, 1.0], tex_coord: [1.0, 0.0]},
    ];

    let vertex_buf = device.create_buffer_with_data(
        bytemuck::cast_slice(&vertices),
        wgpu::BufferUsage::VERTEX
    );

    let vs = include_bytes!("../shaders/shader.vert.spv");
    let vs_module = device.create_shader_module(&wgpu::read_spirv(std::io::Cursor::new(&vs[..])).unwrap());

    let fs = include_bytes!("../shaders/shader.frag.spv");
    let fs_module = device.create_shader_module(&wgpu::read_spirv(std::io::Cursor::new(&fs[..])).unwrap());

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        layout: &pipeline_layout,
        vertex_stage: wgpu::ProgrammableStageDescriptor {
            module: &vs_module,
            entry_point: "main",
        },
        fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
            module: &fs_module,
            entry_point: "main",
        }),
        rasterization_state: Some(wgpu::RasterizationStateDescriptor {
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: wgpu::CullMode::None,
            depth_bias: 0,
            depth_bias_slope_scale: 0.0,
            depth_bias_clamp: 0.0,
        }),
        primitive_topology: wgpu::PrimitiveTopology::TriangleStrip,
        color_states: &[wgpu::ColorStateDescriptor {
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            color_blend: wgpu::BlendDescriptor::REPLACE,
            alpha_blend: wgpu::BlendDescriptor::REPLACE,
            write_mask: wgpu::ColorWrite::ALL,
        }],
        depth_stencil_state: None,
        vertex_state: wgpu::VertexStateDescriptor {
            index_format: wgpu::IndexFormat::Uint16,
            vertex_buffers: &[wgpu::VertexBufferDescriptor {
                stride: std::mem::size_of::<Vertex>() as u64,
                step_mode: wgpu::InputStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttributeDescriptor {
                        format: wgpu::VertexFormat::Float2,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttributeDescriptor {
                        format: wgpu::VertexFormat::Float2,
                        offset: 4 * 2,
                        shader_location: 1,
                    },
                ]
            }],
        },
        sample_count: 1,
        sample_mask: !0,
        alpha_to_coverage_enabled: false,
    });

    let mut last_frame_time = chrono::Utc::now();
    let nanos = 1_000_000_000 / 60;
    let frame_time = chrono::Duration::nanoseconds(nanos);

    // AUDIO
    let audio_stream = make_audio_stream(&mut gba);
    if !mute {
        audio_stream.play().expect("Couldn't start audio stream");
    }
    
    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::LoopDestroyed => (),//debug::debug_mode(&mut gba),
            Event::MainEventsCleared => {
                let now = chrono::Utc::now();
                let since_last = now.signed_duration_since(last_frame_time);
                if since_last < frame_time {
                    return;
                }
                //println!("Frame time: {}", since_last);
                last_frame_time = now;

                let mut buf = device.create_buffer_mapped(&wgpu::BufferDescriptor {
                    label: None,
                    size: (render_size.0 * render_size.1 * 4) as u64,
                    usage: wgpu::BufferUsage::COPY_SRC | wgpu::BufferUsage::MAP_WRITE
                });

                gba.frame(&mut buf.data);

                let tex_buffer = buf.finish();

                let frame = swapchain.get_next_texture().expect("Timeout when acquiring next swapchain tex.");
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {label: None});

                encoder.copy_buffer_to_texture(
                    wgpu::BufferCopyView {
                        buffer: &tex_buffer,
                        offset: 0,
                        bytes_per_row: 4 * texture_extent.width,
                        rows_per_image: 0
                    },
                    wgpu::TextureCopyView {
                        texture: &texture,
                        mip_level: 0,
                        array_layer: 0,
                        origin: wgpu::Origin3d::ZERO,
                    },
                    texture_extent
                );

                {
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                            attachment: &frame.view,
                            resolve_target: None,
                            load_op: wgpu::LoadOp::Clear,
                            store_op: wgpu::StoreOp::Store,
                            clear_color: wgpu::Color::WHITE,
                        }],
                        depth_stencil_attachment: None,
                    });
                    rpass.set_pipeline(&render_pipeline);
                    rpass.set_bind_group(0, &bind_group, &[]);
                    rpass.set_vertex_buffer(0, &vertex_buf, 0, 0);
                    rpass.draw(0..4, 0..1);
                }

                queue.submit(&[encoder.finish()]);
            },
            Event::WindowEvent {
                window_id: _,
                event: w,
            } => match w {
                WindowEvent::CloseRequested => {
                    ::std::process::exit(0);
                },
                WindowEvent::KeyboardInput {
                    device_id: _,
                    input: k,
                    is_synthetic: _,
                } => {
                    let pressed = match k.state {
                        ElementState::Pressed => true,
                        ElementState::Released => false,
                    };
                    match k.virtual_keycode {
                        Some(VirtualKeyCode::Q)         => {
                            *control_flow = ControlFlow::Exit;
                        },
                        Some(VirtualKeyCode::X)         => gba.set_button(gba::Button::A, pressed),
                        Some(VirtualKeyCode::Z)         => gba.set_button(gba::Button::B, pressed),
                        Some(VirtualKeyCode::A)         => gba.set_button(gba::Button::L, pressed),
                        Some(VirtualKeyCode::S)         => gba.set_button(gba::Button::R, pressed),
                        Some(VirtualKeyCode::Space)     => gba.set_button(gba::Button::Select, pressed),
                        Some(VirtualKeyCode::Return)    => gba.set_button(gba::Button::Start, pressed),
                        Some(VirtualKeyCode::Up)        => gba.set_button(gba::Button::Up, pressed),
                        Some(VirtualKeyCode::Down)      => gba.set_button(gba::Button::Down, pressed),
                        Some(VirtualKeyCode::Left)      => gba.set_button(gba::Button::Left, pressed),
                        Some(VirtualKeyCode::Right)     => gba.set_button(gba::Button::Right, pressed),
                        _ => {},
                    }
                },
                WindowEvent::Resized(size) => {
                    swapchain_desc.width = size.width;
                    swapchain_desc.height = size.height;
                    swapchain = device.create_swap_chain(&surface, &swapchain_desc);
                },
                _ => {}
            },
            //Event::RedrawRequested(_) => {},
            _ => {},
        }
    });
}

fn make_audio_stream(gba: &mut gba::GBA) -> cpal::Stream {
    use cpal::traits::{
        DeviceTrait,
        HostTrait
    };

    let host = cpal::default_host();
    let device = host.default_output_device().expect("no output device available.");

    let config = super::pick_output_config(&device).with_max_sample_rate();
    let sample_rate = config.sample_rate().0 as f64;
    println!("Audio sample rate {}", sample_rate);
    let mut audio_handler = gba.enable_audio(sample_rate).unwrap();

    device.build_output_stream(
        &config.into(),
        move |data: &mut [f32], _| {
            audio_handler.get_audio_packet(data);
        },
        move |err| {
            println!("Error occurred: {}", err);
        }
    ).unwrap()
}
