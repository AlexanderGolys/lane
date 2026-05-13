//! Builds preview shaders and runs optional Vulkan preview support.
//! Preview logic is separate from the compiler passes because it deals with shader packaging, graphics API setup, timing, and runtime diagnostics rather than Lane semantics.
//! It is an outer tooling/backend subsystem invoked from API or CLI after GLSL has been generated.

use ash::vk;
use std::error::Error;
use std::ffi::CString;
use std::mem;
use std::time::{Duration, Instant};
use winit::dpi::PhysicalSize;
use winit::event::{KeyEvent, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::{Fullscreen, Window};

pub struct PreviewShaders {
    pub vertex_spv: Vec<u8>,
    pub fragment_spv: Vec<u8>,
}

const FRAME_INTERVAL: Duration = Duration::from_millis(33);

#[repr(C)]
#[derive(Clone, Copy)]
struct PreviewPushConstants {
    camera_position: [f32; 4],
    camera_forward: [f32; 4],
    camera_global_up: [f32; 4],
    resolution: [f32; 2],
    _resolution_pad: [f32; 2],
    ambient_color: [f32; 3],
    time: f32,
}

impl PreviewPushConstants {
    /// Builds push constants for the current frame from viewport size and elapsed time.
    fn new(width: u32, height: u32, time: f32) -> Self {
        Self {
            camera_position: [0.0, 0.0, -1.35, 0.0],
            camera_forward: [0.0, 0.0, 1.0, 0.0],
            camera_global_up: [0.0, 1.0, 0.0, 0.0],
            resolution: [width as f32, height as f32],
            _resolution_pad: [0.0, 0.0],
            ambient_color: [0.035, 0.04, 0.055],
            time,
        }
    }

    /// Returns the packed byte view required by Vulkan push-constant uploads.
    fn bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                (self as *const Self).cast(),
                mem::size_of::<PreviewPushConstants>(),
            )
        }
    }
}

/// Runs the Vulkan preview window and drives frame rendering until close or exit.
pub fn run(shaders: PreviewShaders) -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new()?;
    let window_attributes = Window::default_attributes()
        .with_title("Lane Preview")
        .with_inner_size(PhysicalSize::new(960, 540))
        .with_fullscreen(Some(Fullscreen::Borderless(None)));
    #[allow(deprecated)]
    let window = event_loop.create_window(window_attributes)?;
    let mut renderer = unsafe { Renderer::new(&event_loop, &window, shaders)? };

    let start = Instant::now();
    let mut next_frame = Instant::now();
    #[allow(deprecated)]
    event_loop.run(move |event, event_loop| match event {
        winit::event::Event::WindowEvent { event, .. } => match event {
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::Escape),
                        ..
                    },
                ..
            } => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    renderer.needs_recreate = true;
                    next_frame = Instant::now();
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if renderer.needs_recreate {
                    let size = window.inner_size();
                    if size.width > 0 && size.height > 0 {
                        if let Err(err) =
                            unsafe { renderer.recreate_swapchain(size.width, size.height) }
                        {
                            eprintln!("preview swapchain recreate error: {err}");
                            event_loop.exit();
                            return;
                        }
                    }
                }
                match unsafe { renderer.draw(start.elapsed().as_secs_f32()) } {
                    Ok(DrawStatus::Rendered) => {}
                    Ok(DrawStatus::NeedsRecreate) => {
                        renderer.needs_recreate = true;
                        next_frame = Instant::now();
                        window.request_redraw();
                    }
                    Err(err) => {
                        eprintln!("preview render error: {err}");
                        event_loop.exit();
                    }
                }
                next_frame = Instant::now() + FRAME_INTERVAL;
            }
            _ => {}
        },
        winit::event::Event::AboutToWait => {
            let now = Instant::now();
            if now >= next_frame {
                window.request_redraw();
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(next_frame));
        }
        _ => {}
    })?;
    Ok(())
}

struct Renderer {
    _entry: ash::Entry,
    instance: ash::Instance,
    surface_loader: ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
    device: ash::Device,
    swapchain_loader: ash::khr::swapchain::Device,
    surface_format: vk::SurfaceFormatKHR,
    swapchain: vk::SwapchainKHR,
    queue: vk::Queue,
    extent: vk::Extent2D,
    render_pass: vk::RenderPass,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    framebuffers: Vec<vk::Framebuffer>,
    image_views: Vec<vk::ImageView>,
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    image_available: vk::Semaphore,
    render_finished: vk::Semaphore,
    in_flight: vk::Fence,
    vertex_spv: Vec<u8>,
    fragment_spv: Vec<u8>,
    needs_recreate: bool,
}

struct SwapchainResources {
    swapchain: vk::SwapchainKHR,
    extent: vk::Extent2D,
    render_pass: vk::RenderPass,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    framebuffers: Vec<vk::Framebuffer>,
    image_views: Vec<vk::ImageView>,
}

enum DrawStatus {
    Rendered,
    NeedsRecreate,
}

impl Renderer {
    /// Creates all core Vulkan objects (instance, device, swapchain, and pipeline).
    unsafe fn new(
        event_loop: &EventLoop<()>,
        window: &winit::window::Window,
        shaders: PreviewShaders,
    ) -> Result<Self, Box<dyn Error>> {
        let entry = unsafe { ash::Entry::load()? };
        let app_name = CString::new("lane-preview")?;
        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .application_version(0)
            .engine_name(&app_name)
            .engine_version(0)
            .api_version(vk::make_api_version(0, 1, 0, 0));
        let extension_names =
            ash_window::enumerate_required_extensions(event_loop.display_handle()?.as_raw())?;
        let instance_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(extension_names);
        let instance = unsafe { entry.create_instance(&instance_info, None)? };
        let surface = unsafe {
            ash_window::create_surface(
                &entry,
                &instance,
                window.display_handle()?.as_raw(),
                window.window_handle()?.as_raw(),
                None,
            )?
        };
        let surface_loader = ash::khr::surface::Instance::new(&entry, &instance);

        let (physical_device, queue_family_index) =
            unsafe { select_physical_device(&instance, &surface_loader, surface)? };
        let queue_priority = [1.0];
        let queue_info = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family_index)
            .queue_priorities(&queue_priority)];
        let device_extensions = [ash::khr::swapchain::NAME.as_ptr()];
        let device_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_info)
            .enabled_extension_names(&device_extensions);
        let device = unsafe { instance.create_device(physical_device, &device_info, None)? };
        let queue = unsafe { device.get_device_queue(queue_family_index, 0) };
        let swapchain_loader = ash::khr::swapchain::Device::new(&instance, &device);

        let surface_format =
            unsafe { choose_surface_format(&surface_loader, physical_device, surface)? };
        let size = window.inner_size();
        let swapchain_context = SwapchainCreateContext {
            device: &device,
            swapchain_loader: &swapchain_loader,
            surface_loader: &surface_loader,
            physical_device,
            surface,
            surface_format,
            vertex_spv: &shaders.vertex_spv,
            fragment_spv: &shaders.fragment_spv,
        };
        let resources = unsafe {
            create_swapchain_resources(
                &swapchain_context,
                size.width,
                size.height,
                vk::SwapchainKHR::null(),
            )?
        };
        let command_pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(queue_family_index)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let command_pool = unsafe { device.create_command_pool(&command_pool_info, None)? };
        let command_buffer_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        let command_buffer = unsafe { device.allocate_command_buffers(&command_buffer_info)?[0] };
        let semaphore_info = vk::SemaphoreCreateInfo::default();
        let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
        let image_available = unsafe { device.create_semaphore(&semaphore_info, None)? };
        let render_finished = unsafe { device.create_semaphore(&semaphore_info, None)? };
        let in_flight = unsafe { device.create_fence(&fence_info, None)? };

        Ok(Self {
            _entry: entry,
            instance,
            surface_loader,
            surface,
            physical_device,
            device,
            swapchain_loader,
            surface_format,
            swapchain: resources.swapchain,
            queue,
            extent: resources.extent,
            render_pass: resources.render_pass,
            pipeline_layout: resources.pipeline_layout,
            pipeline: resources.pipeline,
            framebuffers: resources.framebuffers,
            image_views: resources.image_views,
            command_pool,
            command_buffer,
            image_available,
            render_finished,
            in_flight,
            vertex_spv: shaders.vertex_spv,
            fragment_spv: shaders.fragment_spv,
            needs_recreate: false,
        })
    }

    /// Rebuilds swapchain state and dependent objects after size changes or errors.
    unsafe fn recreate_swapchain(&mut self, width: u32, height: u32) -> Result<(), Box<dyn Error>> {
        let swapchain_context = SwapchainCreateContext {
            device: &self.device,
            swapchain_loader: &self.swapchain_loader,
            surface_loader: &self.surface_loader,
            physical_device: self.physical_device,
            surface: self.surface,
            surface_format: self.surface_format,
            vertex_spv: &self.vertex_spv,
            fragment_spv: &self.fragment_spv,
        };
        let resources = unsafe {
            create_swapchain_resources(&swapchain_context, width, height, self.swapchain)?
        };
        unsafe {
            self.device.device_wait_idle()?;
            let old_resources = self.replace_swapchain_resources(resources);
            self.destroy_swapchain_resources(old_resources);
        }
        self.needs_recreate = false;
        Ok(())
    }

    /// Submits one render frame and reports whether swapchain recreation is required.
    unsafe fn draw(&mut self, time: f32) -> Result<DrawStatus, Box<dyn Error>> {
        unsafe {
            self.device
                .wait_for_fences(&[self.in_flight], true, u64::MAX)?;
        }
        let (image_index, suboptimal) = match unsafe {
            self.swapchain_loader.acquire_next_image(
                self.swapchain,
                u64::MAX,
                self.image_available,
                vk::Fence::null(),
            )
        } {
            Ok(result) => result,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => return Ok(DrawStatus::NeedsRecreate),
            Err(err) => return Err(err.into()),
        };
        unsafe {
            self.device.reset_fences(&[self.in_flight])?;
            self.device
                .reset_command_buffer(self.command_buffer, vk::CommandBufferResetFlags::empty())?;
            self.record_command_buffer(image_index as usize, time)?;
        }
        let wait_semaphores = [self.image_available];
        let signal_semaphores = [self.render_finished];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let command_buffers = [self.command_buffer];
        let submit_info = [vk::SubmitInfo::default()
            .wait_semaphores(&wait_semaphores)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(&command_buffers)
            .signal_semaphores(&signal_semaphores)];
        unsafe {
            self.device
                .queue_submit(self.queue, &submit_info, self.in_flight)?;
        }
        let swapchains = [self.swapchain];
        let image_indices = [image_index];
        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&signal_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);
        unsafe {
            match self
                .swapchain_loader
                .queue_present(self.queue, &present_info)
            {
                Ok(present_suboptimal) if suboptimal || present_suboptimal => {
                    return Ok(DrawStatus::NeedsRecreate);
                }
                Ok(_) => {}
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => return Ok(DrawStatus::NeedsRecreate),
                Err(err) => return Err(err.into()),
            }
        }
        Ok(DrawStatus::Rendered)
    }

    /// Records command buffer work for one fullscreen draw call into the swapchain image.
    unsafe fn record_command_buffer(
        &self,
        framebuffer_index: usize,
        time: f32,
    ) -> Result<(), Box<dyn Error>> {
        let begin_info = vk::CommandBufferBeginInfo::default();
        unsafe {
            self.device
                .begin_command_buffer(self.command_buffer, &begin_info)?;
        }
        let clear_values = [vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [0.0, 0.0, 0.0, 1.0],
            },
        }];
        let render_pass_info = vk::RenderPassBeginInfo::default()
            .render_pass(self.render_pass)
            .framebuffer(self.framebuffers[framebuffer_index])
            .render_area(vk::Rect2D::default().extent(self.extent))
            .clear_values(&clear_values);
        let push_constants = PreviewPushConstants::new(self.extent.width, self.extent.height, time);
        unsafe {
            self.device.cmd_begin_render_pass(
                self.command_buffer,
                &render_pass_info,
                vk::SubpassContents::INLINE,
            );
            self.device.cmd_bind_pipeline(
                self.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline,
            );
            self.device.cmd_push_constants(
                self.command_buffer,
                self.pipeline_layout,
                vk::ShaderStageFlags::FRAGMENT,
                0,
                push_constants.bytes(),
            );
            self.device.cmd_draw(self.command_buffer, 3, 1, 0, 0);
            self.device.cmd_end_render_pass(self.command_buffer);
            self.device.end_command_buffer(self.command_buffer)?;
        }
        Ok(())
    }

    /// Swaps active swapchain resource handles with a freshly-created set.
    fn replace_swapchain_resources(&mut self, resources: SwapchainResources) -> SwapchainResources {
        SwapchainResources {
            swapchain: mem::replace(&mut self.swapchain, resources.swapchain),
            extent: mem::replace(&mut self.extent, resources.extent),
            render_pass: mem::replace(&mut self.render_pass, resources.render_pass),
            pipeline_layout: mem::replace(&mut self.pipeline_layout, resources.pipeline_layout),
            pipeline: mem::replace(&mut self.pipeline, resources.pipeline),
            framebuffers: mem::replace(&mut self.framebuffers, resources.framebuffers),
            image_views: mem::replace(&mut self.image_views, resources.image_views),
        }
    }

    /// Detaches current swapchain resources so they can be destroyed independently.
    unsafe fn current_swapchain_resources(&mut self) -> SwapchainResources {
        SwapchainResources {
            swapchain: mem::replace(&mut self.swapchain, vk::SwapchainKHR::null()),
            extent: self.extent,
            render_pass: mem::replace(&mut self.render_pass, vk::RenderPass::null()),
            pipeline_layout: mem::replace(&mut self.pipeline_layout, vk::PipelineLayout::null()),
            pipeline: mem::replace(&mut self.pipeline, vk::Pipeline::null()),
            framebuffers: mem::take(&mut self.framebuffers),
            image_views: mem::take(&mut self.image_views),
        }
    }

    /// Destroys framebuffers, pipeline, and other resources tied to one swapchain set.
    unsafe fn destroy_swapchain_resources(&self, mut resources: SwapchainResources) {
        unsafe {
            for framebuffer in resources.framebuffers.drain(..) {
                self.device.destroy_framebuffer(framebuffer, None);
            }
            self.device.destroy_pipeline(resources.pipeline, None);
            self.device
                .destroy_pipeline_layout(resources.pipeline_layout, None);
            self.device.destroy_render_pass(resources.render_pass, None);
            for view in resources.image_views.drain(..) {
                self.device.destroy_image_view(view, None);
            }
            self.swapchain_loader
                .destroy_swapchain(resources.swapchain, None);
        }
    }
}

impl Drop for Renderer {
    /// Cleans up Vulkan resources in shutdown order; best-effort errors are ignored.
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_fence(self.in_flight, None);
            self.device.destroy_semaphore(self.render_finished, None);
            self.device.destroy_semaphore(self.image_available, None);
            let resources = self.current_swapchain_resources();
            self.destroy_swapchain_resources(resources);
            self.device.destroy_command_pool(self.command_pool, None);
            self.device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}

/// Finds a Vulkan physical device with graphics and present queue support.
unsafe fn select_physical_device(
    instance: &ash::Instance,
    surface_loader: &ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
) -> Result<(vk::PhysicalDevice, u32), Box<dyn Error>> {
    for physical_device in unsafe { instance.enumerate_physical_devices()? } {
        let queue_families =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
        for (index, family) in queue_families.iter().enumerate() {
            let graphics = family.queue_flags.contains(vk::QueueFlags::GRAPHICS);
            let present = unsafe {
                surface_loader.get_physical_device_surface_support(
                    physical_device,
                    index as u32,
                    surface,
                )?
            };
            if graphics && present {
                return Ok((physical_device, index as u32));
            }
        }
    }
    Err("no Vulkan device with graphics+present support".into())
}

/// Selects a usable surface format, favoring sRGB + non-linear color space.
unsafe fn choose_surface_format(
    surface_loader: &ash::khr::surface::Instance,
    physical_device: vk::PhysicalDevice,
    surface: vk::SurfaceKHR,
) -> Result<vk::SurfaceFormatKHR, Box<dyn Error>> {
    let formats =
        unsafe { surface_loader.get_physical_device_surface_formats(physical_device, surface)? };
    Ok(formats
        .iter()
        .copied()
        .find(|format| {
            format.format == vk::Format::B8G8R8A8_SRGB
                && format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
        })
        .unwrap_or(formats[0]))
}

/// Selects a present mode known to be widely supported.
unsafe fn choose_present_mode(
    _surface_loader: &ash::khr::surface::Instance,
    _physical_device: vk::PhysicalDevice,
    _surface: vk::SurfaceKHR,
) -> Result<vk::PresentModeKHR, Box<dyn Error>> {
    Ok(vk::PresentModeKHR::FIFO)
}

struct SwapchainCreateContext<'a> {
    device: &'a ash::Device,
    swapchain_loader: &'a ash::khr::swapchain::Device,
    surface_loader: &'a ash::khr::surface::Instance,
    physical_device: vk::PhysicalDevice,
    surface: vk::SurfaceKHR,
    surface_format: vk::SurfaceFormatKHR,
    vertex_spv: &'a [u8],
    fragment_spv: &'a [u8],
}

/// Creates swapchain plus all dependent render resources for a given extent and assets.
unsafe fn create_swapchain_resources(
    context: &SwapchainCreateContext<'_>,
    width: u32,
    height: u32,
    old_swapchain: vk::SwapchainKHR,
) -> Result<SwapchainResources, Box<dyn Error>> {
    let capabilities = unsafe {
        context
            .surface_loader
            .get_physical_device_surface_capabilities(context.physical_device, context.surface)?
    };
    let extent = choose_extent(&capabilities, width.max(1), height.max(1));
    let present_mode = unsafe {
        choose_present_mode(
            context.surface_loader,
            context.physical_device,
            context.surface,
        )?
    };
    let min_image_count = if capabilities.max_image_count > 0 {
        (capabilities.min_image_count + 1).min(capabilities.max_image_count)
    } else {
        capabilities.min_image_count + 1
    };
    let swapchain_info = vk::SwapchainCreateInfoKHR::default()
        .surface(context.surface)
        .min_image_count(min_image_count)
        .image_format(context.surface_format.format)
        .image_color_space(context.surface_format.color_space)
        .image_extent(extent)
        .image_array_layers(1)
        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
        .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
        .pre_transform(capabilities.current_transform)
        .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
        .present_mode(present_mode)
        .clipped(true)
        .old_swapchain(old_swapchain);
    let swapchain = unsafe {
        context
            .swapchain_loader
            .create_swapchain(&swapchain_info, None)?
    };
    let images = unsafe { context.swapchain_loader.get_swapchain_images(swapchain)? };
    let image_views = images
        .iter()
        .map(|image| unsafe {
            create_image_view(context.device, *image, context.surface_format.format)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let render_pass = unsafe { create_render_pass(context.device, context.surface_format.format)? };
    let (pipeline_layout, pipeline) = unsafe {
        create_pipeline(
            context.device,
            render_pass,
            extent,
            context.vertex_spv,
            context.fragment_spv,
        )?
    };
    let framebuffers = image_views
        .iter()
        .map(|view| unsafe { create_framebuffer(context.device, render_pass, *view, extent) })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(SwapchainResources {
        swapchain,
        extent,
        render_pass,
        pipeline_layout,
        pipeline,
        framebuffers,
        image_views,
    })
}

/// Chooses a valid framebuffer extent, clamped to physical device constraints.
fn choose_extent(
    capabilities: &vk::SurfaceCapabilitiesKHR,
    width: u32,
    height: u32,
) -> vk::Extent2D {
    if capabilities.current_extent.width != u32::MAX {
        return capabilities.current_extent;
    }
    vk::Extent2D {
        width: width.clamp(
            capabilities.min_image_extent.width,
            capabilities.max_image_extent.width,
        ),
        height: height.clamp(
            capabilities.min_image_extent.height,
            capabilities.max_image_extent.height,
        ),
    }
}

/// Creates an image view from a swapchain image for sampling as a color attachment.
unsafe fn create_image_view(
    device: &ash::Device,
    image: vk::Image,
    format: vk::Format,
) -> Result<vk::ImageView, Box<dyn Error>> {
    let range = vk::ImageSubresourceRange::default()
        .aspect_mask(vk::ImageAspectFlags::COLOR)
        .base_mip_level(0)
        .level_count(1)
        .base_array_layer(0)
        .layer_count(1);
    let info = vk::ImageViewCreateInfo::default()
        .image(image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(format)
        .subresource_range(range);
    Ok(unsafe { device.create_image_view(&info, None)? })
}

/// Builds a single-pass render pass used by the preview presentation pipeline.
unsafe fn create_render_pass(
    device: &ash::Device,
    format: vk::Format,
) -> Result<vk::RenderPass, Box<dyn Error>> {
    let attachment = [vk::AttachmentDescription::default()
        .format(format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)];
    let color_ref = [vk::AttachmentReference::default()
        .attachment(0)
        .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
    let subpass = [vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&color_ref)];
    let dependency = [vk::SubpassDependency::default()
        .src_subpass(vk::SUBPASS_EXTERNAL)
        .dst_subpass(0)
        .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
        .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)];
    let info = vk::RenderPassCreateInfo::default()
        .attachments(&attachment)
        .subpasses(&subpass)
        .dependencies(&dependency);
    Ok(unsafe { device.create_render_pass(&info, None)? })
}

/// Creates graphics pipeline objects for fullscreen rendering with default push-constant layout.
unsafe fn create_pipeline(
    device: &ash::Device,
    render_pass: vk::RenderPass,
    extent: vk::Extent2D,
    vertex_spv: &[u8],
    fragment_spv: &[u8],
) -> Result<(vk::PipelineLayout, vk::Pipeline), Box<dyn Error>> {
    let vertex_module = unsafe { create_shader_module(device, vertex_spv)? };
    let fragment_module = unsafe { create_shader_module(device, fragment_spv)? };
    let main = CString::new("main")?;
    let stages = [
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vertex_module)
            .name(&main),
        vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(fragment_module)
            .name(&main),
    ];
    let vertex_input = vk::PipelineVertexInputStateCreateInfo::default();
    let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
        .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
    let viewport = [vk::Viewport::default()
        .x(0.0)
        .y(0.0)
        .width(extent.width as f32)
        .height(extent.height as f32)
        .min_depth(0.0)
        .max_depth(1.0)];
    let scissor = [vk::Rect2D::default().extent(extent)];
    let viewport_state = vk::PipelineViewportStateCreateInfo::default()
        .viewports(&viewport)
        .scissors(&scissor);
    let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
        .polygon_mode(vk::PolygonMode::FILL)
        .line_width(1.0)
        .cull_mode(vk::CullModeFlags::NONE)
        .front_face(vk::FrontFace::COUNTER_CLOCKWISE);
    let multisample = vk::PipelineMultisampleStateCreateInfo::default()
        .rasterization_samples(vk::SampleCountFlags::TYPE_1);
    let color_blend_attachment = [vk::PipelineColorBlendAttachmentState::default()
        .color_write_mask(
            vk::ColorComponentFlags::R
                | vk::ColorComponentFlags::G
                | vk::ColorComponentFlags::B
                | vk::ColorComponentFlags::A,
        )];
    let color_blend =
        vk::PipelineColorBlendStateCreateInfo::default().attachments(&color_blend_attachment);
    let push_ranges = [vk::PushConstantRange::default()
        .stage_flags(vk::ShaderStageFlags::FRAGMENT)
        .offset(0)
        .size(mem::size_of::<PreviewPushConstants>() as u32)];
    let layout_info = vk::PipelineLayoutCreateInfo::default().push_constant_ranges(&push_ranges);
    let pipeline_layout = unsafe { device.create_pipeline_layout(&layout_info, None)? };
    let pipeline_info = [vk::GraphicsPipelineCreateInfo::default()
        .stages(&stages)
        .vertex_input_state(&vertex_input)
        .input_assembly_state(&input_assembly)
        .viewport_state(&viewport_state)
        .rasterization_state(&rasterizer)
        .multisample_state(&multisample)
        .color_blend_state(&color_blend)
        .layout(pipeline_layout)
        .render_pass(render_pass)
        .subpass(0)];
    let pipeline = unsafe {
        device
            .create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_info, None)
            .map_err(|(_, err)| err)?[0]
    };
    unsafe {
        device.destroy_shader_module(fragment_module, None);
        device.destroy_shader_module(vertex_module, None);
    }
    Ok((pipeline_layout, pipeline))
}

/// Creates a Vulkan shader module from SPIR-V bytes.
unsafe fn create_shader_module(
    device: &ash::Device,
    spv: &[u8],
) -> Result<vk::ShaderModule, Box<dyn Error>> {
    let code = ash::util::read_spv(&mut std::io::Cursor::new(spv))?;
    let info = vk::ShaderModuleCreateInfo::default().code(&code);
    Ok(unsafe { device.create_shader_module(&info, None)? })
}

/// Creates a framebuffer for one swapchain image view.
unsafe fn create_framebuffer(
    device: &ash::Device,
    render_pass: vk::RenderPass,
    view: vk::ImageView,
    extent: vk::Extent2D,
) -> Result<vk::Framebuffer, Box<dyn Error>> {
    let attachments = [view];
    let info = vk::FramebufferCreateInfo::default()
        .render_pass(render_pass)
        .attachments(&attachments)
        .width(extent.width)
        .height(extent.height)
        .layers(1);
    Ok(unsafe { device.create_framebuffer(&info, None)? })
}
