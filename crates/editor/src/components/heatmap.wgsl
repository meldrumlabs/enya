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

// Color palette for heatmap (8 colors from cold to hot)
fn get_heatmap_color(value: f32) -> vec4<f32> {
    // Clamp value to 0-1 range
    let t = clamp(value, 0.0, 1.0);

    // Viridis-inspired palette (perceptually uniform)
    let c0 = vec3<f32>(0.267, 0.004, 0.329); // Dark purple
    let c1 = vec3<f32>(0.282, 0.140, 0.458); // Purple
    let c2 = vec3<f32>(0.254, 0.265, 0.530); // Blue-purple
    let c3 = vec3<f32>(0.190, 0.407, 0.556); // Blue
    let c4 = vec3<f32>(0.127, 0.566, 0.551); // Teal
    let c5 = vec3<f32>(0.204, 0.718, 0.473); // Green
    let c6 = vec3<f32>(0.565, 0.843, 0.262); // Yellow-green
    let c7 = vec3<f32>(0.993, 0.906, 0.144); // Yellow

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
