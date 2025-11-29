use spa::{ds, gba, Coords, Device};

use winit::{
    application::ApplicationHandler, dpi::{
        LogicalSize, Size, PhysicalSize
    }, event::{
        ElementState, WindowEvent, MouseButton
    }, event_loop::{
        EventLoop
    }, window::Window, keyboard::{PhysicalKey, KeyCode}
};

use cpal::traits::StreamTrait;

const FRAME_TIME: chrono::Duration = chrono::Duration::nanoseconds(1_000_000_000 / 60);

struct WindowState {
    window:         std::sync::Arc<Window>,
    surface:        wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
}

impl WindowState {
    fn resize_surface(&mut self, size: PhysicalSize<u32>, device: &wgpu::Device) {
        self.surface_config.width = size.width;
        self.surface_config.height = size.height;
        self.surface.configure(device, &self.surface_config);
    }
}

struct App {
    window: Option<WindowState>,
    console: Box<dyn spa::Device>,

    // WGPU params
    instance:        wgpu::Instance,
    adapter:         wgpu::Adapter,
    device:          wgpu::Device,
    queue:           wgpu::Queue,
    texture_extent:  wgpu::Extent3d,
    texture:         wgpu::Texture,
    bind_group:      wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,

    upper_screen_buffer: Vec<u8>,
    lower_screen_buffer: Vec<u8>,
    frame_buffer:        Vec<u8>,
    last_frame_time: chrono::DateTime<chrono::Utc>,

    clicked: bool,
    coords:  Option<spa::Coords<f64>>,

    audio_stream: cpal::Stream
}

impl App {
    fn new(console: Box<dyn spa::Device>, audio_stream: cpal::Stream) -> Self {
        // Setup wgpu
        let instance = wgpu::Instance::new(&Default::default());

        let adapter = futures::executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: None,
        })).expect("Failed to find appropriate adapter");

        let (device, queue) = futures::executor::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            ..Default::default()
        })).expect("Failed to create device");

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
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None
                },
            ]
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[]
        });

        let [upper_render_size, lower_render_size] = console.render_size();
        let texture_extent = wgpu::Extent3d {
            width: upper_render_size.x as u32,
            height: (upper_render_size.y + lower_render_size.y) as u32,
            depth_or_array_layers: 1
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: texture_extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[wgpu::TextureFormat::Rgba8UnormSrgb]
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

        let shader_module = device.create_shader_module(wgpu::include_wgsl!("./shaders/shader.wgsl"));

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default()
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                .. Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default()
            }),
            multiview: None,
            cache: None
        });

        let upper_screen_tex_size = upper_render_size.x * upper_render_size.y * 4;
        let lower_screen_tex_size = lower_render_size.x * lower_render_size.y * 4;
        
        Self {
            window: None,
            console,

            instance,
            adapter,
            device,
            queue,
            texture_extent,
            texture,
            bind_group,
            render_pipeline,

            upper_screen_buffer: vec![0_u8; upper_screen_tex_size],
            lower_screen_buffer: vec![0_u8; lower_screen_tex_size],
            frame_buffer:        Vec::new(),
            last_frame_time: chrono::Utc::now(),

            clicked: false,
            coords: None,

            audio_stream: audio_stream
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let [upper_render_size, lower_render_size] = self.console.render_size();
        let width = upper_render_size.x;
        let height = upper_render_size.y + lower_render_size.y;
        let window_attrs = Window::default_attributes()
            .with_inner_size(Size::Logical(LogicalSize{width: (width * 2) as f64, height: (height * 2) as f64}))
            .with_title("SPA");
        let window = std::sync::Arc::new(event_loop.create_window(window_attrs).unwrap());

        // Setup wgpu
        let surface = self.instance.create_surface(window.clone()).expect("Failed to create surface");

        let size = window.inner_size();
        let surface_config = surface.get_default_config(&self.adapter, size.width, size.height).expect("Could not get default surface config");
        surface.configure(&self.device, &surface_config);

        self.window = Some(WindowState {
            window, surface, surface_config
        });

        self.last_frame_time = chrono::Utc::now();
    
        // AUDIO
        self.audio_stream.play().expect("Couldn't start audio stream");

        //let mut in_focus = true;
    }

    fn window_event(
            &mut self,
            event_loop: &winit::event_loop::ActiveEventLoop,
            _window_id: winit::window::WindowId,
            event: WindowEvent,
        ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            },
            WindowEvent::Resized(size) => {
                self.window.as_mut().unwrap().resize_surface(size, &self.device);
            },
            WindowEvent::RedrawRequested => {
                let now = chrono::Utc::now();
                if now.signed_duration_since(self.last_frame_time) >= FRAME_TIME {
                    self.last_frame_time = now;
    
                    self.console.frame(&mut self.upper_screen_buffer, &mut self.lower_screen_buffer);
    
                    self.frame_buffer.clear();
                    self.frame_buffer.extend_from_slice(&self.upper_screen_buffer);
                    self.frame_buffer.extend_from_slice(&self.lower_screen_buffer);

                    self.queue.write_texture(
                        self.texture.as_image_copy(),
                        &self.frame_buffer, 
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(4 * self.texture_extent.width),
                            rows_per_image: None,
                        },
                        self.texture_extent
                    );
    
                    let frame = self.window.as_ref().unwrap().surface.get_current_texture().expect("Timeout when acquiring next swapchain tex.");
                    let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {label: None});
    
                    {
                        let view = frame.texture.create_view(&Default::default());
                        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: None,
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                                    store: wgpu::StoreOp::Store,
                                },
                                depth_slice: None,
                                resolve_target: None,
                            })],
                            depth_stencil_attachment: None,
                            ..Default::default()
                        });
                        rpass.set_pipeline(&self.render_pipeline);
                        rpass.set_bind_group(0, &self.bind_group, &[]);
                        rpass.draw(0..4, 0..1);
                    }
    
                    self.queue.submit([encoder.finish()]);
                    frame.present();
                }
                self.window.as_ref().unwrap().window.request_redraw();
            },
            WindowEvent::KeyboardInput { device_id: _, event, is_synthetic: _ } => {
                let pressed = match event.state {
                    ElementState::Pressed => true,
                    ElementState::Released => false,
                };
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::KeyX)        => self.console.set_button(spa::Button::A, pressed),
                    PhysicalKey::Code(KeyCode::KeyZ)        => self.console.set_button(spa::Button::B, pressed),
                    PhysicalKey::Code(KeyCode::KeyD)        => self.console.set_button(spa::Button::X, pressed),
                    PhysicalKey::Code(KeyCode::KeyC)        => self.console.set_button(spa::Button::Y, pressed),
                    PhysicalKey::Code(KeyCode::KeyA)        => self.console.set_button(spa::Button::L, pressed),
                    PhysicalKey::Code(KeyCode::KeyS)        => self.console.set_button(spa::Button::R, pressed),
                    PhysicalKey::Code(KeyCode::Space)       => self.console.set_button(spa::Button::Select, pressed),
                    PhysicalKey::Code(KeyCode::Enter)       => self.console.set_button(spa::Button::Start, pressed),
                    PhysicalKey::Code(KeyCode::ArrowUp)     => self.console.set_button(spa::Button::Up, pressed),
                    PhysicalKey::Code(KeyCode::ArrowDown)   => self.console.set_button(spa::Button::Down, pressed),
                    PhysicalKey::Code(KeyCode::ArrowLeft)   => self.console.set_button(spa::Button::Left, pressed),
                    PhysicalKey::Code(KeyCode::ArrowRight)  => self.console.set_button(spa::Button::Right, pressed),
                    _ => {},
                }
            },
            WindowEvent::CursorMoved {
                position: winit::dpi::PhysicalPosition {x, y},
                ..
            } => {
                let y = y / (self.window.as_ref().unwrap().window.inner_size().height as f64);
                if y >= 0.5 {
                    let x = x / (self.window.as_ref().unwrap().window.inner_size().width as f64);
                    let y = (y - 0.5) * 2.0;
                    self.coords = Some(Coords{x, y});
                } else {
                    self.coords = None;
                }
                if self.clicked {
                    self.console.touchscreen_pressed(self.coords);
                }
            },
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                match state {
                    ElementState::Pressed => {
                        self.console.touchscreen_pressed(self.coords);
                        self.clicked = true;
                    },
                    ElementState::Released => {
                        self.console.touchscreen_pressed(None);
                        self.clicked = false;
                    },
                }
            },
            /*WindowEvent::Focused(focused) => {
                in_focus = focused;
                if !in_focus {
                    audio_stream.pause().expect("Couldn't pause audio stream");
                } else {
                    audio_stream.play().expect("Couldn't restart audio stream");
                }
            },*/
            _ => {}
        }
    }
}

pub fn run_nds(config: ds::MemoryConfig, mute: bool) {
    let mut nds: Box<dyn Device> = Box::new(ds::NDS::new(config));

    let audio_stream = make_audio_stream(&mut nds, mute);

    let event_loop = EventLoop::new().expect("Failed to create event loop");

    let mut app = App::new(nds, audio_stream);
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    event_loop.run_app(&mut app).unwrap();
}

pub fn run_gba(config: gba::MemoryConfig, mute: bool) {
    let mut gba: Box<dyn Device> = Box::new(gba::GBA::new(config));

    let audio_stream = make_audio_stream(&mut gba, mute);

    let event_loop = EventLoop::new().expect("Failed to create event loop");

    let mut app = App::new(gba, audio_stream);
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    event_loop.run_app(&mut app).unwrap();
}

fn make_audio_stream(console: &mut Box<dyn Device>, mute: bool) -> cpal::Stream {
    use cpal::traits::{
        DeviceTrait,
        HostTrait
    };

    let host = cpal::default_host();
    let device = host.default_output_device().expect("no output device available.");

    let config = pick_output_config(&device).with_max_sample_rate();
    let sample_rate = config.sample_rate().0 as f64;
    println!("Audio sample rate {}", sample_rate);
    let mut audio_handler = console.enable_audio(sample_rate).unwrap();

    device.build_output_stream(
        &config.into(),
        move |data: &mut [f32], _| {
            audio_handler.get_audio_packet(data);
            if mute {
                for d in data.iter_mut() {
                    *d = 0.0;
                }
            }
        },
        move |err| {
            println!("Error occurred: {}", err);
        }
    ).unwrap()
}

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
