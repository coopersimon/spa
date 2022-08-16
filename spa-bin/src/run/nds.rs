use spa::ds;

use wgpu::util::DeviceExt;
use winit::{
    dpi::{
        Size, LogicalSize, PhysicalPosition
    },
    event::{
        Event, WindowEvent,
        ElementState,
        VirtualKeyCode, MouseButton,
    },
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder
};
//use cpal::traits::StreamTrait;
use super::Vertex;

pub fn run_nds(config: ds::MemoryConfig, _mute: bool) {

    let mut nds = ds::NDS::new(config);
    let render_size = nds.render_size();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size(Size::Logical(LogicalSize{width: (render_size.0 * 2) as f64, height: (render_size.1 * 2 * 2) as f64}))
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
        features: wgpu::Features::SPIRV_SHADER_PASSTHROUGH,
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
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
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
        height: (render_size.1 * 2) as u32,
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

    let vs_module = unsafe {device.create_shader_module_spirv(&wgpu::include_spirv_raw!("../shaders/shader.vert.spv"))};

    let fs_module = unsafe {device.create_shader_module_spirv(&wgpu::include_spirv_raw!("../shaders/shader.frag.spv"))};

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

    // IMGUI
    // TODO: debug only?
    let mut imgui = imgui::Context::create();
    let mut platform = imgui_winit_support::WinitPlatform::init(&mut imgui);
    platform.attach_window(
        imgui.io_mut(),
        &window,
        imgui_winit_support::HiDpiMode::Default
    );
    //imgui.set_ini_filename(None);

    // TODO: HI_DPI
    let font_size = 13.0 * 2.0;
    imgui.io_mut().font_global_scale = (1.0 / 2.0) as f32;

    imgui.fonts().add_font(&[imgui::FontSource::DefaultFontData {
        config: Some(imgui::FontConfig{
            oversample_h: 1,
            pixel_snap_h: true,
            size_pixels: font_size,
            .. Default::default()
        })
    }]);

    let renderer_config = imgui_wgpu::RendererConfig {
        texture_format: surface_config.format,
        .. Default::default()
    };
    let mut renderer = imgui_wgpu::Renderer::new(&mut imgui, &device, &queue, renderer_config);

    let mut last_frame_time = chrono::Utc::now();
    let nanos = 1_000_000_000 / 60;
    let frame_time = chrono::Duration::nanoseconds(nanos);
    let mut frames = vec![0.0; 120];
    let mut frame_idx = 0;

    // AUDIO
    //let audio_stream = make_audio_stream(&mut gba);
    //if !mute {
    //    audio_stream.play().expect("Couldn't start audio stream");
    //}

    let screen_tex_size = render_size.0 * render_size.1 * 4;
    let mut upper_buffer = vec![0_u8; screen_tex_size];
    let mut lower_buffer = vec![0_u8; screen_tex_size];
    let mut screen_buffer = Vec::new();

    let mut clicked = false;
    let mut coords = None;
    
    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::LoopDestroyed => (),//debug::debug_mode(&mut nds),
            Event::MainEventsCleared => {
                let now = chrono::Utc::now();
                let since_last = now.signed_duration_since(last_frame_time);
                if since_last < frame_time {
                    return;
                }
                frames[frame_idx] = since_last.num_nanoseconds().unwrap() as f64;
                frame_idx = (frame_idx + 1) % 120;
                let avg_time = frames.iter().fold(0.0, |acc, n| acc + n) / (frames.len() as f64);
                last_frame_time = now;

                nds.frame(&mut upper_buffer, &mut lower_buffer);
                
                screen_buffer.clear();
                screen_buffer.extend_from_slice(&upper_buffer);
                screen_buffer.extend_from_slice(&lower_buffer);
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

                /*let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: &data,
                    usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::MAP_WRITE
                });
                encoder.copy_buffer_to_texture(
                    wgpu::ImageCopyBuffer {
                        layout: wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: std::num::NonZeroU32::new(4 * texture_extent.width),
                            rows_per_image: std::num::NonZeroU32::new(0),
                        },
                        buffer: &buf,
                    },
                    wgpu::ImageCopyTexture {
                        texture: &texture,
                        mip_level: 0,
                        aspect: wgpu::TextureAspect::All,
                        origin: wgpu::Origin3d::ZERO,
                    },
                    texture_extent
                );*/

                platform.prepare_frame(imgui.io_mut(), &window)
                    .expect("Failed to prepare frame");
                let ui = imgui.frame();

                {
                    let _window = imgui::Window::new("Debug")
                        .size([100.0, 100.0], imgui::Condition::FirstUseEver)
                        .build(&ui, || {
                            ui.text(format!("FPS: {:.1}", 1_000_000_000.0 / avg_time));
                        });
                }

                platform.prepare_render(&ui, &window);

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

                    renderer.render(ui.render(), &queue, &device, &mut rpass)
                        .expect("Cannot render debug UI");
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
                WindowEvent::CursorMoved {
                    position: PhysicalPosition {x, y},
                    ..
                } => {
                    let y = y / (window.inner_size().height as f64);
                    if y >= 0.5 {
                        let x = x / (window.inner_size().width as f64);
                        let y = (y - 0.5) * 2.0;
                        coords = Some((x, y));
                    } else {
                        coords = None;
                    }
                    if clicked {
                        nds.touchscreen_pressed(coords);
                    }
                },
                WindowEvent::MouseInput {
                    state,
                    button: MouseButton::Left,
                    ..
                } => {
                    match state {
                        ElementState::Pressed => {
                            nds.touchscreen_pressed(coords);
                            clicked = true;
                        },
                        ElementState::Released => {
                            nds.touchscreen_pressed(None);
                            clicked = false;
                        },
                    }
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
                        Some(VirtualKeyCode::X)         => nds.set_button(ds::Button::A, pressed),
                        Some(VirtualKeyCode::Z)         => nds.set_button(ds::Button::B, pressed),
                        Some(VirtualKeyCode::D)         => nds.set_button(ds::Button::X, pressed),
                        Some(VirtualKeyCode::C)         => nds.set_button(ds::Button::Y, pressed),
                        Some(VirtualKeyCode::A)         => nds.set_button(ds::Button::L, pressed),
                        Some(VirtualKeyCode::S)         => nds.set_button(ds::Button::R, pressed),
                        Some(VirtualKeyCode::Space)     => nds.set_button(ds::Button::Select, pressed),
                        Some(VirtualKeyCode::Return)    => nds.set_button(ds::Button::Start, pressed),
                        Some(VirtualKeyCode::Up)        => nds.set_button(ds::Button::Up, pressed),
                        Some(VirtualKeyCode::Down)      => nds.set_button(ds::Button::Down, pressed),
                        Some(VirtualKeyCode::Left)      => nds.set_button(ds::Button::Left, pressed),
                        Some(VirtualKeyCode::Right)     => nds.set_button(ds::Button::Right, pressed),
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

/*fn make_audio_stream(nds: &mut nds::GBA) -> cpal::Stream {
    use cpal::traits::{
        DeviceTrait,
        HostTrait
    };

    let host = cpal::default_host();
    let device = host.default_output_device().expect("no output device available.");

    let config = super::pick_output_config(&device).with_max_sample_rate();
    let sample_rate = config.sample_rate().0 as f64;
    println!("Audio sample rate {}", sample_rate);
    let mut audio_handler = nds.enable_audio(sample_rate).unwrap();

    device.build_output_stream(
        &config.into(),
        move |data: &mut [f32], _| {
            audio_handler.get_audio_packet(data);
        },
        move |err| {
            println!("Error occurred: {}", err);
        }
    ).unwrap()
}*/
