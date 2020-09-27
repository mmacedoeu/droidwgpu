// use ndk::trace;
use async_mutex::Mutex;
use std::sync::Arc;
use wgpu::{
    Adapter, Device, Instance, PipelineLayout, Queue, RenderPipeline, ShaderModule, Surface,
    SwapChain, SwapChainDescriptor,
};
use winit::{
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

pub struct WgpuContext {
    instance: Instance,
    stage: WgpuStage,
}

pub enum WgpuStage {
    Init,
    Ready(InnerContext),
}

impl WgpuStage {
    pub fn not_ready(&self) -> bool {
        match self {
            WgpuStage::Ready(_) => false,
            _ => true,
        }
    }
}

pub struct InnerContext {
    surface: Option<Surface>,
    adapter: Adapter,
    device: Device,
    queue: Queue,
    vs_module: ShaderModule,
    fs_module: ShaderModule,
    pipeline_layout: PipelineLayout,
    render_pipeline: RenderPipeline,
    sc_desc: SwapChainDescriptor,
    swap_chain: SwapChain,
}

async fn setup(
    context: &mut Arc<Mutex<WgpuContext>>,
    window: Arc<Window>,
    swapchain_format: &wgpu::TextureFormat,
) {
    let mut unlocked_context = context.lock().await;
    #[cfg(target_os = "android")]
    let init = unlocked_context.stage.not_ready() && ndk_glue::native_window().as_ref().is_some();
    #[cfg(not(target_os = "android"))]
    let init = unlocked_context.stage.not_ready();
    #[cfg(target_os = "android")]
    println!(
        "setup start, native_window: {}",
        ndk_glue::native_window().as_ref().is_some()
    );
    let ref mut ctx = *unlocked_context;
    match ctx.stage {
        WgpuStage::Init => {
            let surface = if init {
                Some(unsafe { ctx.instance.create_surface(&*window) })
            } else {
                None
            };
            if let Some(ref s) = surface {
                let adapter = ctx
                    .instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::Default,
                        // Request an adapter which can render to our surface
                        compatible_surface: Some(s),
                    })
                    .await
                    .expect("Failed to find an appropriate adapter");

                println!("Adapter: \t {:?}", adapter.get_info());
                // Create the logical device and command queue
                let (device, queue) = adapter
                    .request_device(
                        &wgpu::DeviceDescriptor {
                            features: wgpu::Features::empty(),
                            limits: wgpu::Limits::default(),
                            shader_validation: true,
                        },
                        None,
                    )
                    .await
                    .expect("Failed to create device");

                // Load the shaders from disk
                println!("Device created, loading shaders");
                let vs_module =
                    device.create_shader_module(wgpu::include_spirv!("shader.vert.spv"));
                let fs_module =
                    device.create_shader_module(wgpu::include_spirv!("shader.frag.spv"));

                println!("shaders created, loading pipeline layout");
                let pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: None,
                        bind_group_layouts: &[],
                        push_constant_ranges: &[],
                    });

                println!("shaders created, loading pipeline layout");
                let render_pipeline =
                    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: None,
                        layout: Some(&pipeline_layout),
                        vertex_stage: wgpu::ProgrammableStageDescriptor {
                            module: &vs_module,
                            entry_point: "main",
                        },
                        fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                            module: &fs_module,
                            entry_point: "main",
                        }),
                        // Use the default rasterizer state: no culling, no depth bias
                        rasterization_state: None,
                        primitive_topology: wgpu::PrimitiveTopology::TriangleList,
                        color_states: &[(*swapchain_format).into()],
                        depth_stencil_state: None,
                        vertex_state: wgpu::VertexStateDescriptor {
                            index_format: wgpu::IndexFormat::Uint16,
                            vertex_buffers: &[],
                        },
                        sample_count: 1,
                        sample_mask: !0,
                        alpha_to_coverage_enabled: false,
                    });

                let size = window.inner_size();
                let sc_desc = wgpu::SwapChainDescriptor {
                    usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
                    format: swapchain_format.clone(),
                    width: size.width,
                    height: size.height,
                    present_mode: wgpu::PresentMode::Mailbox,
                };

                let swap_chain = device.create_swap_chain(s, &sc_desc);

                ctx.stage = WgpuStage::Ready(InnerContext {
                    surface,
                    adapter,
                    device,
                    queue,
                    vs_module,
                    fs_module,
                    pipeline_layout,
                    render_pipeline,
                    sc_desc,
                    swap_chain,
                });
                println!("setup ok");
            }
        }
        WgpuStage::Ready(ref mut inner) => {
            #[cfg(target_os = "android")]
            let init = inner.surface.is_none() && ndk_glue::native_window().as_ref().is_some();
            #[cfg(not(target_os = "android"))]
            let init = inner.surface.is_none();
            if init {
                let surface = if init {
                    Some(unsafe { ctx.instance.create_surface(&*window) })
                } else {
                    None
                };
                inner.surface = surface;
            } 
            // else {
            //     inner.device.
            // }
        }
    }
}

async fn clean_surface(context: &mut Arc<Mutex<WgpuContext>>) {
    let mut unlocked_context = context.lock().await;
    let ref mut ctx = *unlocked_context;
    match ctx.stage {
        WgpuStage::Ready(ref mut inner) => {
            let _ = inner.surface.take();
        }
        WgpuStage::Init => {}
    };
}

async fn draw(context: &mut Arc<Mutex<WgpuContext>>) {
    println!("draw");
    let mut unlocked_context = context.lock().await;
    let ref mut ctx = *unlocked_context;
    match ctx.stage {
        WgpuStage::Ready(ref mut ready) => {
            let frame = ready
                .swap_chain
                .get_current_frame()
                .expect("Failed to acquire next swap chain texture")
                .output;

            let mut encoder = ready
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            {
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                        attachment: &frame.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                            store: true,
                        },
                    }],
                    depth_stencil_attachment: None,
                });

                rpass.set_pipeline(&ready.render_pipeline);
                rpass.draw(0..3, 0..1);
            }

            ready.queue.submit(Some(encoder.finish()));
        }
        WgpuStage::Init => {
            println!("got draw Init");
        }
    }
}

fn run(event_loop: EventLoop<()>, window: Arc<Window>, swapchain_format: wgpu::TextureFormat) {
    let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);
    let wgpucontext = WgpuContext {
        instance,
        stage: WgpuStage::Init,
    };
    let guard = Arc::new(Mutex::new(wgpucontext));
    let cloned_window = window.clone();

    event_loop.run(move |event, _, control_flow| {
        println!("{:?}", event);
        *control_flow = ControlFlow::Wait;
        match event {
            Event::NewEvents(StartCause::Init) => {
                let mut cg1 = guard.clone();
                let cw = cloned_window.clone();
                let _t = smol::block_on(async move {
                    setup(
                        &mut cg1,
                        cw.clone(),
                        &swapchain_format,
                    )
                    .await;
                    println!("got StartCause::Init:");
                });              
            }
            Event::Resumed => {
                let mut cg1 = guard.clone();
                let cw = cloned_window.clone();
                let _t = smol::block_on(async move {
                    setup(
                        &mut cg1,
                        cw.clone(),
                        &swapchain_format,
                    ).await;
                    draw(&mut cg1).await;
                    println!("got Resumed");
                });
            }
            Event::Suspended => {
                // let mut cg1 = guard.clone();
                // let _t = smol::block_on(async move {
                //     clean_surface(&mut cg1).await;
                //     println!("got Suspended");
                // });
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                println!("got Resized:");
                // Recreate the swap chain with the new size
                let cg1 = guard.clone();
                let _t = smol::block_on(async move {
                    let context = cg1.clone();
                    let mut unlocked_context = context.lock().await;
                    let ref mut ctx = *unlocked_context;
                    match ctx.stage {
                        WgpuStage::Ready(ref mut ready) => {
                            println!("got Resized Ready: \t {}", ready.surface.is_some());
                            if let Some(ref surface) = &ready.surface {
                                ready.sc_desc.width = size.width;
                                ready.sc_desc.height = size.height;
                                ready.swap_chain =
                                    ready.device.create_swap_chain(surface, &ready.sc_desc);
                                println!("Resized:");
                            }
                        }
                        WgpuStage::Init => {
                            println!("got Resized: Init");
                            // setup(
                            //     &mut cloned_guard.clone(),
                            //     cloned_window.clone(),
                            //     &swapchain_format,
                            // )
                            // .await;
                            // draw(&mut cloned_guard.clone()).await;
                        }
                    }
                });
                println!("Resized end");
            }
            Event::MainEventsCleared => {
            }
            Event::RedrawRequested(_) => {
                println!("got RedrawRequested:");
                let mut cg1 = guard.clone();
                let cw = cloned_window.clone();
                let _t = smol::block_on(async move {
                    setup(
                        &mut cg1,
                        cw.clone(),
                        &swapchain_format,
                    )
                    .await;
                    draw(&mut cg1).await;
                });
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => {}
        }
    });
}

#[cfg(target_os = "android")]
ndk_glue::ndk_glue!(main);

// #[cfg_attr(target_os = "android", ndk_glue::main(backtrace = "on"))]
fn main() {
    // let _trace;
    // if trace::is_trace_enabled() {
    //     _trace = trace::Section::new("ndk-rs example main").unwrap();
    // }
    println!("dispositivo test 9:");

    let event_loop = EventLoop::new();
    let window = winit::window::Window::new(&event_loop).unwrap();
    #[cfg(not(target_arch = "wasm32"))]
    {
        subscriber::initialize_default_subscriber(None);
        // Temporarily avoid srgb formats for the swapchain on the web
        run(
            event_loop,
            Arc::new(window),
            wgpu::TextureFormat::Rgba8Unorm,
        );
    }
}
