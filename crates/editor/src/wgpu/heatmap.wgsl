// Heatmap GPU Shader
// Renders a grid of colored cells based on data values

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

struct Uniforms {
    // Transform from normalized coords to clip space
    transform: mat4x4<f32>,
    // Number of columns and rows
    grid_size: vec2<f32>,
    // Cell size in normalized space
    cell_size: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// Cell data: packed as (row, col, value, _padding)
@group(0) @binding(1)
var<storage, read> cell_data: array<vec4<f32>>;

// Color palette for heatmap - Obsidian Glass emerald theme
// Dark-to-emerald gradient matching the editor's brand colors
fn get_heatmap_color(value: f32) -> vec4<f32> {
    // Clamp value to 0-1 range
    let t = clamp(value, 0.0, 1.0);

    // Obsidian Glass emerald palette (dark to bright emerald)
    let c0 = vec3<f32>(0.039, 0.039, 0.039); // bg::BASE - almost black (#0A0A0A)
    let c1 = vec3<f32>(0.078, 0.110, 0.098); // Dark with subtle green tint
    let c2 = vec3<f32>(0.071, 0.149, 0.125); // accent::MUTED - subtle emerald
    let c3 = vec3<f32>(0.078, 0.235, 0.196); // Deeper emerald
    let c4 = vec3<f32>(0.125, 0.392, 0.333); // Mid teal-emerald
    let c5 = vec3<f32>(0.063, 0.549, 0.392); // Approaching accent
    let c6 = vec3<f32>(0.063, 0.725, 0.506); // accent::PRIMARY - emerald (#10B981)
    let c7 = vec3<f32>(0.204, 0.827, 0.600); // accent::HOVER - bright emerald (#34D399)

    // Interpolate between colors
    let segment = t * 7.0;
    let idx = floor(segment);
    let frac = segment - idx;

    var color: vec3<f32>;
    if (idx < 1.0) {
        color = mix(c0, c1, frac);
    } else if (idx < 2.0) {
        color = mix(c1, c2, frac);
    } else if (idx < 3.0) {
        color = mix(c2, c3, frac);
    } else if (idx < 4.0) {
        color = mix(c3, c4, frac);
    } else if (idx < 5.0) {
        color = mix(c4, c5, frac);
    } else if (idx < 6.0) {
        color = mix(c5, c6, frac);
    } else {
        color = mix(c6, c7, frac);
    }

    return vec4<f32>(color, 1.0);
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_idx: u32,
    @builtin(instance_index) instance_idx: u32,
) -> VertexOutput {
    var out: VertexOutput;

    // Get cell data for this instance
    let cell = cell_data[instance_idx];
    let col = cell.x;
    let row = cell.y;
    let value = cell.z;

    // Generate quad vertices (2 triangles = 6 vertices)
    // Vertex order: 0-1-2 (bottom-left triangle), 3-4-5 (top-right triangle)
    var local_pos: vec2<f32>;
    switch (vertex_idx % 6u) {
        case 0u: { local_pos = vec2<f32>(0.0, 0.0); } // bottom-left
        case 1u: { local_pos = vec2<f32>(1.0, 0.0); } // bottom-right
        case 2u: { local_pos = vec2<f32>(0.0, 1.0); } // top-left
        case 3u: { local_pos = vec2<f32>(1.0, 0.0); } // bottom-right
        case 4u: { local_pos = vec2<f32>(1.0, 1.0); } // top-right
        case 5u: { local_pos = vec2<f32>(0.0, 1.0); } // top-left
        default: { local_pos = vec2<f32>(0.0, 0.0); }
    }

    // Calculate cell position with small gap
    let gap = 0.02; // 2% gap between cells
    let effective_cell = uniforms.cell_size * (1.0 - gap);
    let cell_offset = uniforms.cell_size * gap * 0.5;

    // Position in normalized space (0-1)
    let pos = vec2<f32>(
        col * uniforms.cell_size.x + cell_offset.x + local_pos.x * effective_cell.x,
        row * uniforms.cell_size.y + cell_offset.y + local_pos.y * effective_cell.y
    );

    // Transform to clip space
    out.position = uniforms.transform * vec4<f32>(pos, 0.0, 1.0);
    out.color = get_heatmap_color(value);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
