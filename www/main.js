import init, { ParticleGrid } from "./pkg/particle_affinity_wasm.js";

async function initApp() {
  // 1️⃣ Load wasm module
  await init();
  console.log("WASM module loaded successfully");

  // 2️⃣ Setup WebGPU
  const canvas = document.getElementById("gpu-canvas");
  if (!navigator.gpu) {
    console.error("WebGPU not supported");
    return;
  }

  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) {
    console.error("No WebGPU adapter found");
    return;
  }

  const device = await adapter.requestDevice();
  const context = canvas.getContext("webgpu");

  const format = navigator.gpu.getPreferredCanvasFormat();
  context.configure({
    device,
    format,
    alphaMode: "premultiplied",
  });

  // 3️⃣ Load shader - using your existing shader code
  const shaderCode = `
    @group(0) @binding(0)
    var gridTex: texture_2d<u32>;

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
            return vec3f(1.0, 0.4, 0.4); // red
        } else if (t == 2u) {
            return vec3f(0.4, 1.0, 0.4); // green
        } else if (t == 3u) {
            return vec3f(0.4, 0.4, 1.0); // blue
        } else if (t == 4u) {
            return vec3f(1.0, 1.0, 0.4); // yellow
        } else if (t == 5u) {
            return vec3f(1.0, 0.4, 1.0); // magenta
        } else if (t == 6u) {
            return vec3f(0.4, 1.0, 1.0); // cyan
        } else {
            // Enhanced rainbow mapping for higher types
            let hue = f32(t % 12u) / 12.0;
            let s = 0.8;
            let v = 1.0;
            
            let c = v * s;
            let h_prime = hue * 6.0;
            let x = c * (1.0 - abs((h_prime % 2.0) - 1.0));
            let m = v - c;
            
            if h_prime < 1.0 {
                return vec3f(c + m, x + m, m);
            } else if h_prime < 2.0 {
                return vec3f(x + m, c + m, m);
            } else if h_prime < 3.0 {
                return vec3f(m, c + m, x + m);
            } else if h_prime < 4.0 {
                return vec3f(m, x + m, c + m);
            } else if h_prime < 5.0 {
                return vec3f(x + m, m, c + m);
            } else {
                return vec3f(c + m, m, x + m);
            }
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
  `;

  const module = device.createShaderModule({ code: shaderCode });

  // 4️⃣ Create render pipeline
  const pipeline = device.createRenderPipeline({
    layout: "auto",
    vertex: { module, entryPoint: "vs_main" },
    fragment: {
      module,
      entryPoint: "fs_main",
      targets: [{ format }],
    },
    primitive: { topology: "triangle-list" },
  });

  // 5️⃣ Bind group setup - matches original approach
  const bindGroupLayout = pipeline.getBindGroupLayout(0);
  let bindGroup;

  function updateBindGroup() {
    // Only bind the texture - matches WGSL
    bindGroup = device.createBindGroup({
      layout: bindGroupLayout,
      entries: [
        { binding: 0, resource: textureView }
      ],
    });
  }

  // 6️⃣ Prepare GPU resources
  let grid;
  let texture;
  let textureView;

  // 7️⃣ Restart / initialize grid
  function restart() {
    const size = parseInt(document.getElementById("grid-size").value);
    const types = parseInt(document.getElementById("num-types").value);
    const density = parseFloat(document.getElementById("density").value);
    const radius = parseInt(document.getElementById("radius").value);
    const affStr = document.getElementById("affinity").value.trim();

    console.log(`Creating grid: ${size}x${size}, ${types} types, density ${density}, radius ${radius}`);

    let affinity = null;
    if (affStr.length > 0) {
      try {
        affinity = affStr.split(",").map((x) => parseInt(x.trim(), 10));
        console.log("Using custom affinity:", affinity.slice(0, 10), "...");
      } catch (e) {
        console.warn("Invalid affinity string, using random:", e);
        affinity = null;
      }
    }

    // Create new grid from WASM
    grid = new ParticleGrid(size, types, density, radius, affinity);
    console.log("Grid created:", grid.debug_info());

    // Allocate GPU texture
    texture = device.createTexture({
      size: [size, size],
      format: "r8uint",
      usage:
        GPUTextureUsage.COPY_DST |
        GPUTextureUsage.TEXTURE_BINDING |
        GPUTextureUsage.RENDER_ATTACHMENT,
    });

    textureView = texture.createView();
    updateBindGroup();

    // Update UI
    document.getElementById("status").textContent = 
      `${size}×${size} grid with ${types} particle types (density ${(density*100).toFixed(1)}%)`;
  }

  document.getElementById("restart").onclick = restart;
  
  // Initialize with default values
  restart();

  // 8️⃣ Render loop
  let frameCount = 0;
  let lastFpsUpdate = performance.now();

  function frame() {
    // Step simulation in WASM
    grid.step();

    // Get Uint8Array from WASM
    const data = grid.export_grid();

    // Upload data to texture
    device.queue.writeTexture(
      { texture },
      data,
      { bytesPerRow: grid.size },
      [grid.size, grid.size]
    );

    // Encode render pass
    const encoder = device.createCommandEncoder();
    const pass = encoder.beginRenderPass({
      colorAttachments: [
        {
          view: context.getCurrentTexture().createView(),
          loadOp: "clear",
          storeOp: "store",
          clearValue: { r: 0.02, g: 0.02, b: 0.02, a: 1 },
        },
      ],
    });

    pass.setPipeline(pipeline);
    pass.setBindGroup(0, bindGroup);
    pass.draw(6);
    pass.end();

    device.queue.submit([encoder.finish()]);

    // FPS counter
    frameCount++;
    const now = performance.now();
    if (now - lastFpsUpdate > 1000) {
      const fps = Math.round(frameCount / ((now - lastFpsUpdate) / 1000));
      document.getElementById("fps").textContent = `${fps} FPS`;
      frameCount = 0;
      lastFpsUpdate = now;
    }

    requestAnimationFrame(frame);
  }

  console.log("Starting render loop");
  requestAnimationFrame(frame);
}

// Handle errors
window.addEventListener('error', (e) => {
  console.error('Global error:', e.error);
});

window.addEventListener('unhandledrejection', (e) => {
  console.error('Unhandled promise rejection:', e.reason);
});

initApp().catch(console.error);