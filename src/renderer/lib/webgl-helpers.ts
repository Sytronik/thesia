import {COLORMAP_RGBA8} from "../prototypes/constants/colors";

export const MARGIN_FOR_RESIZE = 5; // at least 3 for Lanczos-3 kernel

export const VS_RESIZER = `#version 300 es
precision highp float;

/* locations 0 & 1 match the calls to gl.vertexAttribPointer(...) */
layout(location = 0) in  vec4 aPosition;
layout(location = 1) in  vec2 aTexCoord;

uniform vec2 uStep;          // (1/srcW,0)  or  (0,1/srcH)
uniform float uTexOffset;    // Horizontal offset (srcLeft / texWidth)
uniform float uTexScale;     // Horizontal scale (srcW / texWidth)
uniform float uTexOffsetY;   // Vertical offset (srcTop / texHeight)
uniform float uTexScaleY;    // Vertical scale (srcH / texHeight)

out vec2 vTex[9];            // centre ± 4 offsets

void main() {
    gl_Position = aPosition;

    // Map input aTexCoord [0,1] horizontally to [uTexOffset, uTexOffset + uTexScale]
    // and vertically to [uTexOffsetY, uTexOffsetY + uTexScaleY]
    vec2 baseTexCoord = vec2(
        uTexOffset + aTexCoord.x * uTexScale,
        uTexOffsetY + aTexCoord.y * uTexScaleY
    );

    /* centre sample */
    vTex[4] = baseTexCoord;

    /* ±1 … ±4 samples */
    for (int i = 1; i <= 4; ++i) {
        vec2 o = float(i) * uStep;
        // Steps are calculated relative to the full texture size, so apply directly
        vTex[4 - i] = baseTexCoord - o;
        vTex[4 + i] = baseTexCoord + o;
    }
}`;

export const FS_RESIZER = `#version 300 es
precision highp float;

uniform sampler2D uTex;
uniform vec2 uStep;  // (1/srcW, 0) or (0, 1/srcH) for the current pass
uniform float uScale; // dstSize / srcSize for the current pass

in  vec2 vTex[9]; // vTex[4] is the center coord in source texture space
out float fragColor;

const float PI = 3.141592653589793;
const float a = 3.0; // Lanczos kernel radius

// sinc(x) = sin(pi*x) / (pi*x)
float sinc(float x) {
    if (x == 0.0) {
        return 1.0;
    }
    // Note: HLSL's sinc function is sin(x)/x, but GLSL doesn't have one.
    // We need sin(pi*x)/(pi*x).
    float pix = PI * x;
    return sin(pix) / pix;
}

// Lanczos kernel (a=3)
float lanczos3(float x) {
    if (x == 0.0) {
        return 1.0;
    }
    if (x <= -a || x >= a) {
        return 0.0;
    }
    return sinc(x) * sinc(x / a);
}

void main() {
    vec2 centerCoord = vTex[4];
    float srcCoord; // Coordinate in source pixel space for the relevant dimension
    float step;     // Pixel step size for the relevant dimension

    // Determine dimension based on uStep (non-zero component indicates direction)
    bool isHorizontal = (uStep.y == 0.0);
    if (isHorizontal) {
        // Avoid division by zero if uStep.x is somehow zero
        if (uStep.x == 0.0) { fragColor = texture(uTex, centerCoord).r; return; }
        srcCoord = centerCoord.x / uStep.x;
        step = uStep.x;
    } else {
        // Avoid division by zero if uStep.y is somehow zero
        if (uStep.y == 0.0) { fragColor = texture(uTex, centerCoord).r; return; }
        srcCoord = centerCoord.y / uStep.y;
        step = uStep.y;
    }

    float scale = uScale;
    // Handle potential division by zero or negative scale
    if (scale <= 0.0) { fragColor = texture(uTex, centerCoord).r; return; }

    float effective_radius = a / min(scale, 1.0);

    float weightSum = 0.0;
    float valueSum = 0.0;

    // Calculate integer bounds for the loop
    int start = int(floor(srcCoord - effective_radius));
    int end = int(ceil(srcCoord + effective_radius));

    // Clamp loop iterations for safety, although dynamic loops are fine in GLSL 300 es
    // This depends on the maximum expected downscaling factor.
    // int max_taps = 64; // Example limit
    // start = max(start, int(srcCoord) - max_taps/2);
    // end = min(end, int(srcCoord) + max_taps/2);

    for (int j = start; j <= end; ++j) {
        float pos = (float(j) + 0.5); // Center of source pixel j
        float dist = srcCoord - pos;  // Distance from dst center to src center (in source pixels)

        // Argument for the lanczos function depends on upscaling vs downscaling
        float x_lanczos = dist * min(scale, 1.0);

        float weight = lanczos3(x_lanczos);

        if (weight != 0.0) {
            vec2 sampleCoord;
            if (isHorizontal) {
                sampleCoord = vec2(pos * step, centerCoord.y);
            } else {
                sampleCoord = vec2(centerCoord.x, pos * step);
            }

            // Sample the R channel (luminance) from the R32F source
            float value = texture(uTex, sampleCoord).r;

            valueSum += value * weight;
            weightSum += weight;
        }
    }

    // Normalize the result
    if (weightSum == 0.0 || abs(weightSum) < 1e-6) {
         // Fallback: Sample center pixel directly if weights sum to (near) zero
         fragColor = texture(uTex, centerCoord).r;
    } else {
         fragColor = valueSum / weightSum;
    }

    // Optional: Clamp output if needed, though R32F supports out-of-range values.
    // fragColor = clamp(fragColor, 0.0, 1.0);
}`;

export const VS_COLORMAP = `#version 300 es
layout(location=0) in vec2 aPos;   // fullscreen quad
layout(location=1) in vec2 aUV;
out vec2 vUV;
void main() {
    vUV = aUV;
    gl_Position = vec4(aPos, 0.0, 1.0);
}`;

export const FS_COLORMAP = `#version 300 es
precision highp float;

uniform sampler2D uLum;      // R32F
uniform sampler2D uColorMap; // 256×1 RGBA8
uniform float uOverlayAlpha; // 0.0 (no overlay) to 1.0 (full black)
in  vec2 vUV;
out vec4 fragColor;

void main() {
    float l  = texture(uLum, vUV).r;           // 0-to-1 luminance
    vec4  c  = texture(uColorMap, vec2(l, .5)); // lookup color
    // Mix with black based on uOverlayAlpha
    vec3 finalRgb = mix(c.rgb, vec3(0.0), uOverlayAlpha);
    fragColor = vec4(finalRgb, 1.0);           // solid alpha
}`;

function createShader(gl: WebGL2RenderingContext, type: number, src: string): WebGLShader {
  const s = gl.createShader(type);
  if (!s) throw new Error("Failed to create shader");
  gl.shaderSource(s, src);
  gl.compileShader(s);
  if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw gl.getShaderInfoLog(s);
  return s;
}

function createProgram(gl: WebGL2RenderingContext, vsSrc: string, fsSrc: string): WebGLProgram {
  const p = gl.createProgram();
  gl.attachShader(p, createShader(gl, gl.VERTEX_SHADER, vsSrc));
  gl.attachShader(p, createShader(gl, gl.FRAGMENT_SHADER, fsSrc));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw gl.getProgramInfoLog(p);
  return p;
}

export function createResizeProgram(gl: WebGL2RenderingContext) {
  return createProgram(gl, VS_RESIZER, FS_RESIZER);
}

export function createColormapProgram(gl: WebGL2RenderingContext) {
  return createProgram(gl, VS_COLORMAP, FS_COLORMAP);
}

export function createTexture(
  gl: WebGL2RenderingContext,
  w: number,
  h: number,
  data: Float32Array | Uint8Array | null = null,
  format: number = gl.R32F,
): WebGLTexture {
  const t = gl.createTexture();
  if (!t) throw new Error("Failed to create texture");
  gl.bindTexture(gl.TEXTURE_2D, t);

  // Set common parameters
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);

  if (format === gl.R32F) {
    // Use NEAREST filtering for R32F textures to ensure framebuffer completeness
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    // Upload data or allocate storage
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.R32F, w, h, 0, gl.RED, gl.FLOAT, data);
  } else if (format === gl.RGBA8) {
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    // Upload data or allocate storage
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, data);
  } else {
    gl.deleteTexture(t); // Clean up before throwing
    throw new Error(`Unsupported texture format in createTexture: ${format}`);
  }

  gl.bindTexture(gl.TEXTURE_2D, null); // Unbind
  return t;
}

export function createCmapTexture(gl: WebGL2RenderingContext) {
  const cmapTex = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, cmapTex);
  gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1); // avoid row-stride padding issues  [oai_citation:5‡WebGL2 Fundamentals](https://webgl2fundamentals.org/webgl/lessons/webgl-data-textures.html)
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 256, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, COLORMAP_RGBA8);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  return cmapTex;
}

export type WebGLResources = {
  gl: WebGL2RenderingContext;
  resizeProgram: WebGLProgram;
  colormapProgram: WebGLProgram;
  resizeUniforms: {
    uStep: WebGLUniformLocation | null;
    uTex: WebGLUniformLocation | null;
    uScale: WebGLUniformLocation | null;
    uTexOffset: WebGLUniformLocation | null;
    uTexScale: WebGLUniformLocation | null;
    uTexOffsetY: WebGLUniformLocation | null;
    uTexScaleY: WebGLUniformLocation | null;
  };
  colormapUniforms: {
    uLum: WebGLUniformLocation | null;
    uColorMap: WebGLUniformLocation | null;
    uOverlayAlpha: WebGLUniformLocation | null;
  };
  resizePosBuffer: WebGLBuffer; // Buffer for vertex/UV data used in resize passes
  cmapVao: WebGLVertexArrayObject; // VAO for the colormap pass fullscreen quad
  cmapVbo: WebGLBuffer; // VBO for the colormap pass fullscreen quad
};

export function prepareWebGLResources(canvas: HTMLCanvasElement): WebGLResources | null {
  const gl = canvas.getContext("webgl2", {
    alpha: true,
    antialias: false,
    depth: false,
    preserveDrawingBuffer: true,
    // desynchronized: true,  // cause flickering when resizing on Windows 10
  });

  if (!gl) {
    console.error("Failed to get WebGL2 context.");
    return null;
  }

  // Check for float buffer support
  const ext = gl.getExtension("EXT_color_buffer_float");
  if (!ext) {
    console.warn(
      "WebGL extension 'EXT_color_buffer_float' not supported. " +
        "Rendering to float textures might fail.",
    );
  }

  let resources: WebGLResources | null = null;
  try {
    // --- Create Resize Program and related resources ---
    const resizeProgram = createResizeProgram(gl);
    const resizeUniforms = {
      uStep: gl.getUniformLocation(resizeProgram, "uStep"),
      uTex: gl.getUniformLocation(resizeProgram, "uTex"),
      uScale: gl.getUniformLocation(resizeProgram, "uScale"),
      uTexOffset: gl.getUniformLocation(resizeProgram, "uTexOffset"),
      uTexScale: gl.getUniformLocation(resizeProgram, "uTexScale"),
      uTexOffsetY: gl.getUniformLocation(resizeProgram, "uTexOffsetY"),
      uTexScaleY: gl.getUniformLocation(resizeProgram, "uTexScaleY"),
    };
    const resizePosBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, resizePosBuffer);
    // Data for a quad covering the viewport, including texture coordinates
    gl.bufferData(
      gl.ARRAY_BUFFER,
      new Float32Array([
            // Pos (-1 to 1)  // UV (0 to 1)
            -1, -1, 0, 0, // bottom-left
            1, -1, 1, 0, // bottom-right
            -1, 1, 0, 1, // top-left
            -1, 1, 0, 1, // top-left
            1, -1, 1, 0, // bottom-right
            1, 1, 1, 1, // top-right
          ]), // prettier-ignore
      gl.STATIC_DRAW,
    );
    const aPosResizeLoc = gl.getAttribLocation(resizeProgram, "aPosition");
    const aUVResizeLoc = gl.getAttribLocation(resizeProgram, "aTexCoord");

    // --- Create Colormap Program and related resources ---
    const colormapProgram = createColormapProgram(gl);
    const colormapUniforms = {
      uLum: gl.getUniformLocation(colormapProgram, "uLum"),
      uColorMap: gl.getUniformLocation(colormapProgram, "uColorMap"),
      uOverlayAlpha: gl.getUniformLocation(colormapProgram, "uOverlayAlpha"),
    };
    const cmapVao = gl.createVertexArray();
    gl.bindVertexArray(cmapVao);
    const cmapVbo = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, cmapVbo);
    // Data for a fullscreen quad (using TRIANGLE_STRIP)
    const cmapQuadVertices = new Float32Array([
          // positions // texCoords
          -1.0, 1.0, 0.0, 1.0, // top-left
          -1.0, -1.0, 0.0, 0.0, // bottom-left
          1.0, 1.0, 1.0, 1.0, // top-right
          1.0, -1.0, 1.0, 0.0, // bottom-right
        ]); // prettier-ignore
    gl.bufferData(gl.ARRAY_BUFFER, cmapQuadVertices, gl.STATIC_DRAW);
    const aPosCmapLoc = gl.getAttribLocation(colormapProgram, "aPos");
    const aUVCmapLoc = gl.getAttribLocation(colormapProgram, "aUV");
    gl.enableVertexAttribArray(aPosCmapLoc);
    gl.vertexAttribPointer(aPosCmapLoc, 2, gl.FLOAT, false, 16, 0); // 2 floats position, 4*4=16 bytes stride, 0 offset
    gl.enableVertexAttribArray(aUVCmapLoc);
    gl.vertexAttribPointer(aUVCmapLoc, 2, gl.FLOAT, false, 16, 8); // 2 floats UV, 16 bytes stride, 8 bytes offset
    gl.bindVertexArray(null); // Unbind VAO
    gl.bindBuffer(gl.ARRAY_BUFFER, null); // Unbind VBO

    // Store all successfully created resources
    resources = {
      gl,
      resizeProgram,
      colormapProgram,
      resizeUniforms,
      colormapUniforms,
      resizePosBuffer,
      cmapVao,
      cmapVbo,
    };

    // Setup vertex attributes for the resize program (can be done once)
    gl.bindBuffer(gl.ARRAY_BUFFER, resources.resizePosBuffer);
    gl.enableVertexAttribArray(aPosResizeLoc);
    gl.enableVertexAttribArray(aUVResizeLoc);
    // Stride is 16 bytes (4 floats: PosX, PosY, UVx, UVy), Pos is offset 0, UV is offset 8
    gl.vertexAttribPointer(aPosResizeLoc, 2, gl.FLOAT, false, 16, 0);
    gl.vertexAttribPointer(aUVResizeLoc, 2, gl.FLOAT, false, 16, 8);
    gl.bindBuffer(gl.ARRAY_BUFFER, null); // Unbind buffer
    return resources;
  } catch (error) {
    console.error("Error initializing WebGL resources:", error);
    // Clean up partially created resources if necessary
    if (gl) {
      // Attempt to delete any resources that might have been created before the error
      if (resources) {
        gl.deleteProgram(resources.resizeProgram);
        gl.deleteProgram(resources.colormapProgram);
        gl.deleteBuffer(resources.resizePosBuffer);
        gl.deleteVertexArray(resources.cmapVao);
        gl.deleteBuffer(resources.cmapVbo);
      } else {
        // If webglResourcesRef wasn't set yet, try deleting based on local vars
        // This requires careful handling as some vars might be undefined if error occurred early
        // Example: if (resizeProgram) gl.deleteProgram(resizeProgram); etc.
      }
    }
    return null;
  }
}

export function renderSpectrogram(
  webglResources: WebGLResources,
  spectrogram: Spectrogram,
  srcLeft: number,
  srcTop: number,
  srcW: number,
  srcH: number,
  dstW: number,
  dstH: number,
  blend: number,
) {
  const {
    gl,
    resizeProgram,
    colormapProgram,
    resizeUniforms,
    colormapUniforms,
    resizePosBuffer,
    cmapVao,
  } = webglResources;

  if (blend <= 0) {
    gl.viewport(0, 0, gl.drawingBufferWidth, gl.drawingBufferHeight); // Set viewport to full canvas
    gl.clearColor(0, 0, 0, 0); // Clear full canvas to transparent
    gl.clear(gl.COLOR_BUFFER_BIT);

    if (dstW > 0 && dstH > 0) {
      gl.enable(gl.SCISSOR_TEST); // Enable scissor test
      // Scissor box origin (0,0) is bottom-left corner
      gl.scissor(0, 0, dstW, dstH);
      gl.clearColor(0, 0, 0, 1); // Set clear color to opaque black
      gl.clear(gl.COLOR_BUFFER_BIT); // Clear only the scissor box
      gl.disable(gl.SCISSOR_TEST); // Disable scissor test
    }
    return; // Skip the rest of the rendering pipeline
  }

  // Vertical texture coordinates parameters
  const vTexOffset = srcTop / spectrogram.height; // Offset in normalized coords (0 to 1)
  const vTexScale = srcH / spectrogram.height; // Scale in normalized coords (0 to 1)

  let texSrc: WebGLTexture | null = null;
  let texMid: WebGLTexture | null = null;
  let texResized: WebGLTexture | null = null;
  let fbMid: WebGLFramebuffer | null = null;
  let fbo: WebGLFramebuffer | null = null;
  let cmapTex: WebGLTexture | null = null;

  try {
    // Use Resize Program
    gl.useProgram(resizeProgram);

    // Bind the shared position/UV buffer for resize passes
    gl.bindBuffer(gl.ARRAY_BUFFER, resizePosBuffer);
    // Attribute pointers are already set in canvasElemCallback, assuming VAO is not used here or rebound correctly
    // If you were using a VAO for these attributes, you'd bind it here.

    // Upload source R32F texture
    texSrc = createTexture(gl, spectrogram.width, spectrogram.height, spectrogram.arr, gl.R32F);

    // Create intermediate R32F texture for horizontal pass result
    texMid = createTexture(gl, dstW, srcH, null, gl.R32F);
    fbMid = gl.createFramebuffer();

    // Create final R32F texture for fully resized result
    texResized = createTexture(gl, dstW, dstH, null, gl.R32F);
    fbo = gl.createFramebuffer();

    // Set texture unit 0 for the sampler
    gl.uniform1i(resizeUniforms.uTex, 0);
    gl.activeTexture(gl.TEXTURE0);

    // --- Pass-1 (horizontal resize + vertical crop setup) ---
    const scaleX = dstW / srcW;
    gl.uniform1f(resizeUniforms.uScale, scaleX);
    gl.uniform2f(resizeUniforms.uStep, 1 / spectrogram.width, 0); // Step relative to full src width
    // Horizontal crop uniforms
    gl.uniform1f(resizeUniforms.uTexOffset, srcLeft / spectrogram.width);
    gl.uniform1f(resizeUniforms.uTexScale, srcW / spectrogram.width);
    // Vertical crop uniforms (apply the crop defined by srcTop/srcH)
    gl.uniform1f(resizeUniforms.uTexOffsetY, vTexOffset);
    gl.uniform1f(resizeUniforms.uTexScaleY, vTexScale);

    gl.bindTexture(gl.TEXTURE_2D, texSrc);
    gl.bindFramebuffer(gl.FRAMEBUFFER, fbMid);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, texMid, 0);
    if (gl.checkFramebufferStatus(gl.FRAMEBUFFER) !== gl.FRAMEBUFFER_COMPLETE) {
      throw new Error("Framebuffer 'fbMid' incomplete");
    }
    gl.viewport(0, 0, dstW, srcH); // Viewport for intermediate texture
    gl.drawArrays(gl.TRIANGLES, 0, 6); // Draw quad

    // --- Pass-2 (vertical resize) ---
    const scaleY = dstH / srcH;
    gl.uniform1f(resizeUniforms.uScale, scaleY);
    gl.uniform2f(resizeUniforms.uStep, 0, 1 / srcH); // Vertical step relative to cropped height (srcH)
    // Reset horizontal crop/scale (input tex coords are 0..1 for intermediate tex)
    gl.uniform1f(resizeUniforms.uTexOffset, 0.0);
    gl.uniform1f(resizeUniforms.uTexScale, 1.0);
    // Reset vertical crop/scale (input tex coords are 0..1 for intermediate tex)
    gl.uniform1f(resizeUniforms.uTexOffsetY, 0.0);
    gl.uniform1f(resizeUniforms.uTexScaleY, 1.0);

    gl.bindTexture(gl.TEXTURE_2D, texMid); // Read from intermediate texMid
    gl.bindFramebuffer(gl.FRAMEBUFFER, fbo); // Render to final fbo
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, texResized, 0);
    if (gl.checkFramebufferStatus(gl.FRAMEBUFFER) !== gl.FRAMEBUFFER_COMPLETE) {
      throw new Error("Framebuffer 'fbo' incomplete");
    }
    gl.viewport(0, 0, dstW, dstH); // Set viewport to final destination size
    gl.drawArrays(gl.TRIANGLES, 0, 6); // Draw quad

    // --- Pass-3 Colormap Application ---
    gl.useProgram(colormapProgram);
    gl.bindVertexArray(cmapVao); // Bind the VAO for the fullscreen quad

    cmapTex = createCmapTexture(gl);

    // Calculate overlay alpha based on blend value
    const overlayAlpha = blend < 0.5 ? Math.max(0.0, 1.0 - 2.0 * blend) : 0.0;

    // Setup textures for colormap pass
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, texResized); // Use the final resized R32F texture
    gl.uniform1i(colormapUniforms.uLum, 0);

    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, cmapTex);
    gl.uniform1i(colormapUniforms.uColorMap, 1);

    // Set the overlay alpha uniform
    gl.uniform1f(colormapUniforms.uOverlayAlpha, overlayAlpha);

    // Render to canvas
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.viewport(0, 0, dstW, dstH); // Ensure viewport matches canvas destination size
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    // Attributes are already set up via cmapVao
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4); // Draw the quad using TRIANGLE_STRIP

    // Check for WebGL errors after drawing
    const error = gl.getError();
    if (error !== gl.NO_ERROR) console.error("WebGL Error after draw:", error);
  } catch (error) {
    console.error("Error during WebGL draw:", error);
  } finally {
    // --- Cleanup textures and framebuffers created in this draw call ---
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.bindTexture(gl.TEXTURE_2D, null);
    gl.bindVertexArray(null); // Unbind VAO
    gl.bindBuffer(gl.ARRAY_BUFFER, null); // Unbind any buffers

    if (texSrc) gl.deleteTexture(texSrc);
    if (texMid) gl.deleteTexture(texMid);
    if (texResized) gl.deleteTexture(texResized);
    if (cmapTex) gl.deleteTexture(cmapTex);
    if (fbMid) gl.deleteFramebuffer(fbMid);
    if (fbo) gl.deleteFramebuffer(fbo);
  }
}

export function cleanupWebGLResources(resources: WebGLResources) {
  const {gl, resizeProgram, colormapProgram, resizePosBuffer, cmapVao, cmapVbo} = resources;
  gl.deleteProgram(resizeProgram);
  gl.deleteProgram(colormapProgram);
  gl.deleteBuffer(resizePosBuffer);
  gl.deleteVertexArray(cmapVao);
  gl.deleteBuffer(cmapVbo);
}
