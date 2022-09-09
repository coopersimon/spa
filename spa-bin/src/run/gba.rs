use spa::gba;

use wgpu::util::DeviceExt;
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
    let instance = wgpu::Instance::new(wgpu::Backends::all());
    let surface = unsafe {instance.create_surface(&window)};

    let adapter = futures::executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::default(),
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
    })).expect("Failed to find appropriate adapter");

    let (device, queue) = futures::executor::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: None,
        features: wgpu::Features::default(),
        limits: wgpu::Limits::default()
    }, None)).expect("Failed to create device");
    
    let size = window.inner_size();
    let mut surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::Fifo
    };
    surface.configure(&device, &surface_config);

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler { filtering: true, comparison: false },
                //ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                count: None
            },
        ]
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[]
    });

    let texture_extent = wgpu::Extent3d {
        width: render_size.0 as u32,
        height: render_size.1 as u32,
        depth_or_array_layers: 1
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        size: texture_extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        label: None,
    });
    let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter:     wgpu::FilterMode::Nearest,
        min_filter:     wgpu::FilterMode::Linear,
        mipmap_filter:  wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&texture_view)
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler)
            }
        ],
        label: None
    });

    let vertices = vec![
        Vertex{position: [-1.0, -1.0], tex_coord: [0.0, 1.0]},
        Vertex{position: [1.0, -1.0], tex_coord: [1.0, 1.0]},
        Vertex{position: [-1.0, 1.0], tex_coord: [0.0, 0.0]},
        Vertex{position: [1.0, 1.0], tex_coord: [1.0, 0.0]},
    ];

    let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX
    });

    let vs_module = device.create_shader_module(&wgpu::include_spirv!("../shaders/shader.vert.spv"));
    let fs_module = device.create_shader_module(&wgpu::include_spirv!("../shaders/shader.frag.spv"));

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &vs_module,
            entry_point: "main",
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 4 * 2,
                        shader_location: 1,
                    },
                ]
            }]
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            .. Default::default()
            /*front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,*/
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false
        },
        fragment: Some(wgpu::FragmentState {
            module: &fs_module,
            entry_point: "main",
            targets: &[wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            }]
        }),
        //multiview: None
    });

    let mut last_frame_time = chrono::Utc::now();
    let nanos = 1_000_000_000 / 60;
    let frame_time = chrono::Duration::nanoseconds(nanos);

    // AUDIO
    let audio_stream = make_audio_stream(&mut gba);
    if !mute {
        audio_stream.play().expect("Couldn't start audio stream");
    }
    
    let screen_tex_size = render_size.0 * render_size.1 * 4;
    let mut screen_buffer = vec![0_u8; screen_tex_size];
    
    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::LoopDestroyed => (),//debug::debug_mode(&mut gba),
            Event::MainEventsCleared => {
                let now = chrono::Utc::now();
                let since_last = now.signed_duration_since(last_frame_time);
                if since_last < frame_time {
                    return;
                }
                last_frame_time = now;

                gba.frame(&mut screen_buffer);

                queue.write_texture(
                    texture.as_image_copy(),
                    &screen_buffer, 
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: std::num::NonZeroU32::new(4 * texture_extent.width),
                        rows_per_image: None,
                    },
                    texture_extent
                );

                let frame = surface.get_current_texture().expect("Timeout when acquiring next swapchain tex.");
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {label: None});

                {
                    let view = frame.texture.create_view(&Default::default());
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: None,
                        color_attachments: &[wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                                store: true,
                            },
                        }],
                        depth_stencil_attachment: None,
                    });
                    rpass.set_pipeline(&render_pipeline);
                    rpass.set_bind_group(0, &bind_group, &[]);
                    rpass.set_vertex_buffer(0, vertex_buf.slice(..));
                    rpass.draw(0..4, 0..1);
                }

                queue.submit([encoder.finish()]);
                frame.present();
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
                    surface_config.width = size.width;
                    surface_config.height = size.height;
                    surface.configure(&device, &surface_config);
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
