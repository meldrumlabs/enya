//! GPU-accelerated heatmap rendering using wgpu
//!
//! This module provides GPU resources and rendering callbacks for the heatmap
//! visualization. It uses instanced rendering to efficiently display large grids
//! of colored cells.

use std::sync::Arc;

use eframe::egui_wgpu;

use crate::components::pane::heatmap::HeatmapCell;

/// GPU resources for heatmap rendering
pub struct HeatmapGpuResources {
    pipeline: eframe::wgpu::RenderPipeline,
    uniform_buffer: eframe::wgpu::Buffer,
    cell_buffer: eframe::wgpu::Buffer,
    bind_group: eframe::wgpu::BindGroup,
    max_cells: usize,
}

impl HeatmapGpuResources {
    /// Create new GPU resources for heatmap rendering
    pub fn new(device: &eframe::wgpu::Device, target_format: eframe::wgpu::TextureFormat) -> Self {
        use eframe::wgpu;

        // Load shader from the wgpu module directory
        let shader_source = include_str!("heatmap.wgsl");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Heatmap Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Create uniform buffer (transform + grid info)
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Heatmap Uniforms"),
            size: 96, // 64 (mat4x4) + 8 (vec2) + 8 (vec2) + padding
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create cell buffer (storage buffer for cell data)
        let max_cells = 10000;
        let cell_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Heatmap Cells"),
            size: (max_cells * 16) as u64, // vec4<f32> per cell
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Heatmap Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Heatmap Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: cell_buffer.as_entire_binding(),
                },
            ],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Heatmap Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Heatmap Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            uniform_buffer,
            cell_buffer,
            bind_group,
            max_cells,
        }
    }

    /// Update GPU buffers with cell data
    pub fn prepare(
        &self,
        queue: &eframe::wgpu::Queue,
        cells: &[HeatmapCell],
        grid_size: (usize, usize),
        clip_rect: egui::Rect,
        screen_size: egui::Vec2,
    ) {
        // Build transform matrix (clip space)
        // Map from normalized [0,1] coords to clip space [-1,1]
        let x_scale = 2.0 * clip_rect.width() / screen_size.x;
        let y_scale = 2.0 * clip_rect.height() / screen_size.y;
        let x_offset = 2.0 * clip_rect.left() / screen_size.x - 1.0;
        let y_offset = 1.0 - 2.0 * clip_rect.top() / screen_size.y - y_scale;

        #[rustfmt::skip]
        let transform: [[f32; 4]; 4] = [
            [x_scale, 0.0,      0.0, 0.0],
            [0.0,     -y_scale, 0.0, 0.0],
            [0.0,     0.0,      1.0, 0.0],
            [x_offset, y_offset, 0.0, 1.0],
        ];

        let (cols, rows) = grid_size;
        let cell_size = [1.0 / cols as f32, 1.0 / rows as f32];

        // Pack uniforms
        let mut uniform_data = Vec::with_capacity(96);
        for row in &transform {
            for &val in row {
                uniform_data.extend_from_slice(&val.to_le_bytes());
            }
        }
        uniform_data.extend_from_slice(&(cols as f32).to_le_bytes());
        uniform_data.extend_from_slice(&(rows as f32).to_le_bytes());
        uniform_data.extend_from_slice(&cell_size[0].to_le_bytes());
        uniform_data.extend_from_slice(&cell_size[1].to_le_bytes());
        // Pad to 96 bytes
        while uniform_data.len() < 96 {
            uniform_data.push(0);
        }

        queue.write_buffer(&self.uniform_buffer, 0, &uniform_data);

        // Pack cell data
        let cell_count = cells.len().min(self.max_cells);
        let mut cell_data = Vec::with_capacity(cell_count * 16);
        for cell in cells.iter().take(cell_count) {
            cell_data.extend_from_slice(&(cell.col as f32).to_le_bytes());
            cell_data.extend_from_slice(&(cell.row as f32).to_le_bytes());
            cell_data.extend_from_slice(&cell.value.to_le_bytes());
            cell_data.extend_from_slice(&0.0_f32.to_le_bytes()); // padding
        }

        queue.write_buffer(&self.cell_buffer, 0, &cell_data);
    }

    /// Execute the render pass
    pub fn paint(&self, render_pass: &mut eframe::wgpu::RenderPass<'_>, num_cells: usize) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        // 6 vertices per cell (2 triangles), instanced rendering
        render_pass.draw(0..6, 0..num_cells.min(self.max_cells) as u32);
    }
}

/// Callback for GPU-accelerated heatmap rendering
pub struct HeatmapCallback {
    cells: Arc<Vec<HeatmapCell>>,
    grid_size: (usize, usize),
    clip_rect: egui::Rect,
}

impl HeatmapCallback {
    /// Create a new heatmap callback
    pub fn new(
        cells: Arc<Vec<HeatmapCell>>,
        grid_size: (usize, usize),
        clip_rect: egui::Rect,
    ) -> Self {
        Self {
            cells,
            grid_size,
            clip_rect,
        }
    }
}

impl egui_wgpu::CallbackTrait for HeatmapCallback {
    fn prepare(
        &self,
        _device: &eframe::wgpu::Device,
        queue: &eframe::wgpu::Queue,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut eframe::wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<eframe::wgpu::CommandBuffer> {
        // Get GPU resources (must be initialized via init_heatmap_resources during app startup)
        let Some(gpu_resources) = resources.get::<HeatmapGpuResources>() else {
            // Resources not initialized - GPU rendering not available
            return Vec::new();
        };

        let screen_size = egui::vec2(
            screen_descriptor.size_in_pixels[0] as f32,
            screen_descriptor.size_in_pixels[1] as f32,
        );

        gpu_resources.prepare(
            queue,
            &self.cells,
            self.grid_size,
            self.clip_rect,
            screen_size,
        );

        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut eframe::wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(gpu_resources) = resources.get::<HeatmapGpuResources>() {
            gpu_resources.paint(render_pass, self.cells.len());
        }
    }
}

/// Initialize GPU resources for heatmap rendering
/// Call this once during app initialization when wgpu_render_state is available
pub fn init_heatmap_resources(render_state: &egui_wgpu::RenderState) {
    let device = &render_state.device;
    let format = render_state.target_format;

    render_state
        .renderer
        .write()
        .callback_resources
        .insert(HeatmapGpuResources::new(device, format));
}
