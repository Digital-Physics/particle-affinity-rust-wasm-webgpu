@group(0) @binding(0)
var gridTex: texture_2d<u32>; // our r8uint texture

@group(0) @binding(1)
var gridSampler: sampler;

struct VSOut {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VSOut {
    // full-screen quad
    var positions = array<vec2f, 6>(
        vec2f(-1.0, -1.0),
        vec2f( 1.0, -1.0),
        vec2f(-1.0,  1.0),
        vec2f(-1.0,  1.0),
        vec2f( 1.0, -1.0),
        vec2f( 1.0,  1.0),
    );

    var uvs = array<vec2f, 6>(
        vec2f(0.0, 1.0),
        vec2f(1.0, 1.0),
        vec2f(0.0, 0.0),
        vec2f(0.0, 0.0),
        vec2f(1.0, 1.0),
        vec2f(1.0, 0.0),
    );

    var out: VSOut;
    out.pos = vec4f(positions[vertex_index], 0.0, 1.0);
    out.uv = uvs[vertex_index];
    return out;
}

fn particle_color(t: u32) -> vec3f {
    if (t == 0u) {
        return vec3f(0.0, 0.0, 0.0); // empty = black
    } else if (t == 1u) {
        return vec3f(1.0, 0.0, 0.0); // red
    } else if (t == 2u) {
        return vec3f(0.0, 1.0, 0.0); // green
    } else if (t == 3u) {
        return vec3f(0.0, 0.0, 1.0); // blue
    } else {
        // fallback rainbow mapping for higher types
        let hue = f32(t % 6u) / 6.0;
        return vec3f(hue, 1.0 - hue, 0.5);
    }
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4f {
    // sample from the integer grid texture
    let texSize = textureDimensions(gridTex);
    let uv = in.uv * vec2f(texSize);
    let coord = vec2<i32>(uv);

    let t: u32 = textureLoad(gridTex, coord, 0).r;
    let color = particle_color(t);
    return vec4f(color, 1.0);
}
