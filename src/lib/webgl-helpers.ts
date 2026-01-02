import {COLORMAP_RGBA8} from "../prototypes/constants/colors";

export const MAX_WEBGL_RESOURCES = 16;

export const MAX_TEXTURE_SIZE = getMaxTextureSize();

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

export const VS_BILINEAR_RESIZER = `#version 300 es
precision highp float;

/* locations 0 & 1 match the calls to gl.vertexAttribPointer(...) */
layout(location = 0) in  vec4 aPosition;
layout(location = 1) in  vec2 aTexCoord;

uniform float uTexOffset;    // Horizontal offset (srcLeft / texWidth)
uniform float uTexScale;     // Horizontal scale (srcW / texWidth)
uniform float uTexOffsetY;   // Vertical offset (srcTop / texHeight)
uniform float uTexScaleY;    // Vertical scale (srcH / texHeight)

out vec2 vTexCenter;         // Output only the center texture coordinate

void main() {
    gl_Position = aPosition;

    // Map input aTexCoord [0,1] horizontally to [uTexOffset, uTexOffset + uTexScale]
    // and vertically to [uTexOffsetY, uTexOffsetY + uTexScaleY]
    vTexCenter = vec2(
        uTexOffset + aTexCoord.x * uTexScale,
        uTexOffsetY + aTexCoord.y * uTexScaleY
    );
}`;

export const FS_BILINEAR_RESIZER = `#version 300 es
precision highp float;

uniform sampler2D uTex;
uniform vec2 uStep;  // (1/srcW, 0) or (0, 1/srcH) for the current pass

in  vec2 vTexCenter;
out float fragColor;

void main() {
    vec2 centerCoord = vTexCenter;
    float srcCoord; // Coordinate in source pixel space for the relevant dimension
    float step;     // Pixel step size for the relevant dimension

    // Determine dimension based on uStep (non-zero component indicates direction)
    bool isHorizontal = (uStep.y == 0.0);
    vec2 coord0, coord1;
    float frac;

    if (isHorizontal) {
        // Avoid division by zero if uStep.x is somehow zero
        if (uStep.x == 0.0) { fragColor = texture(uTex, centerCoord).r; return; }
        srcCoord = centerCoord.x / uStep.x;
        step = uStep.x;
        float floor_coord = floor(srcCoord - 0.5); // Integer coordinate of the pixel to the 'left'/'up'
        frac = srcCoord - (floor_coord + 0.5);     // Fractional distance from the center of the left pixel
        coord0 = vec2((floor_coord + 0.5) * step, centerCoord.y); // Center of left pixel
        coord1 = vec2((floor_coord + 1.5) * step, centerCoord.y); // Center of right pixel
    } else {
        // Avoid division by zero if uStep.y is somehow zero
        if (uStep.y == 0.0) { fragColor = texture(uTex, centerCoord).r; return; }
        srcCoord = centerCoord.y / uStep.y;
        step = uStep.y;
        float floor_coord = floor(srcCoord - 0.5); // Integer coordinate of the pixel 'up'
        frac = srcCoord - (floor_coord + 0.5);     // Fractional distance from the center of the upper pixel
        coord0 = vec2(centerCoord.x, (floor_coord + 0.5) * step); // Center of upper pixel
        coord1 = vec2(centerCoord.x, (floor_coord + 1.5) * step); // Center of lower pixel
    }

    // Sample the two nearest pixels in the current dimension
    float v0 = texture(uTex, coord0).r;
    float v1 = texture(uTex, coord1).r;

    // Linear interpolation
    fragColor = mix(v0, v1, frac);

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

function getMaxTextureSize() {
  const canvas = document.createElement("canvas");
  const gl = canvas.getContext("webgl2");
  if (!gl) throw new Error("WebGL2 is not supported");
  const maxTextureSize = gl.getParameter(gl.MAX_TEXTURE_SIZE);
  gl.getExtension("WEBGL_lose_context")?.loseContext();
  return maxTextureSize;
}

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
  if (!p) throw new Error("Failed to create program");
  gl.attachShader(p, createShader(gl, gl.VERTEX_SHADER, vsSrc));
  gl.attachShader(p, createShader(gl, gl.FRAGMENT_SHADER, fsSrc));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) {
    const log = gl.getProgramInfoLog(p);
    gl.deleteProgram(p);
    throw new Error(`Program linking failed: ${log}`);
  }
  return p;
}

export function createResizeProgram(gl: WebGL2RenderingContext) {
  return createProgram(gl, VS_RESIZER, FS_RESIZER);
}

export function createBilinearResizeProgram(gl: WebGL2RenderingContext) {
  return createProgram(gl, VS_BILINEAR_RESIZER, FS_BILINEAR_RESIZER);
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
  bilinearResizeProgram: WebGLProgram;
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
  bilinearResizeUniforms: {
    uStep: WebGLUniformLocation | null;
    uTex: WebGLUniformLocation | null;
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
  resizePosBuffer: WebGLBuffer | null;
  cmapVao: WebGLVertexArrayObject | null;
  cmapVbo: WebGLBuffer | null;
  textureCache: {
    texMid: WebGLTexture | null;
    texResized: WebGLTexture | null;
    fbMid: WebGLFramebuffer | null;
    fbo: WebGLFramebuffer | null;
    cmapTex: WebGLTexture | null;
    lastMidSize: {width: number; height: number} | null;
    lastResizedSize: {width: number; height: number} | null;
  };
};

let numWebGLResources = 0;

export function prepareWebGLResources(canvas: HTMLCanvasElement): WebGLResources | null {
  if (numWebGLResources >= MAX_WEBGL_RESOURCES) {
    console.warn("Too many WebGL resources. Returning null.");
    return null;
  }

  const gl = canvas.getContext("webgl2", {
    alpha: true,
    antialias: false,
    depth: false,
    preserveDrawingBuffer: true,
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

  const resources: Partial<WebGLResources> = {gl}; // Use partial to build up
  try {
    // --- Create Lanczos Resize Program and related resources ---
    resources.resizeProgram = createResizeProgram(gl);
    resources.resizeUniforms = {
      uStep: gl.getUniformLocation(resources.resizeProgram, "uStep"),
      uTex: gl.getUniformLocation(resources.resizeProgram, "uTex"),
      uScale: gl.getUniformLocation(resources.resizeProgram, "uScale"),
      uTexOffset: gl.getUniformLocation(resources.resizeProgram, "uTexOffset"),
      uTexScale: gl.getUniformLocation(resources.resizeProgram, "uTexScale"),
      uTexOffsetY: gl.getUniformLocation(resources.resizeProgram, "uTexOffsetY"),
      uTexScaleY: gl.getUniformLocation(resources.resizeProgram, "uTexScaleY"),
    };
    const aPosResizeLoc = gl.getAttribLocation(resources.resizeProgram, "aPosition");
    const aUVResizeLoc = gl.getAttribLocation(resources.resizeProgram, "aTexCoord");

    // --- Create Bilinear Resize Program ---
    resources.bilinearResizeProgram = createBilinearResizeProgram(gl);
    resources.bilinearResizeUniforms = {
      uStep: gl.getUniformLocation(resources.bilinearResizeProgram, "uStep"),
      uTex: gl.getUniformLocation(resources.bilinearResizeProgram, "uTex"),
      uTexOffset: gl.getUniformLocation(resources.bilinearResizeProgram, "uTexOffset"),
      uTexScale: gl.getUniformLocation(resources.bilinearResizeProgram, "uTexScale"),
      uTexOffsetY: gl.getUniformLocation(resources.bilinearResizeProgram, "uTexOffsetY"),
      uTexScaleY: gl.getUniformLocation(resources.bilinearResizeProgram, "uTexScaleY"),
    };
    // Get attribute locations specific to the bilinear program
    const aPosBilinearLoc = gl.getAttribLocation(resources.bilinearResizeProgram, "aPosition");
    const aUVBilinearLoc = gl.getAttribLocation(resources.bilinearResizeProgram, "aTexCoord");

    // --- Create Shared Resize Buffer (used by both Lanczos and Bilinear) ---
    resources.resizePosBuffer = gl.createBuffer();
    if (!resources.resizePosBuffer) throw new Error("Failed to create resize buffer");
    gl.bindBuffer(gl.ARRAY_BUFFER, resources.resizePosBuffer);
    gl.bufferData(
      gl.ARRAY_BUFFER,
      new Float32Array([
            // Pos (-1 to 1)  // UV (0 to 1)
            -1, -1, 0, 0, // bottom-left
             1, -1, 1, 0, // bottom-right
            -1,  1, 0, 1, // top-left
            -1,  1, 0, 1, // top-left
             1, -1, 1, 0, // bottom-right
             1,  1, 1, 1, // top-right
          ]), // prettier-ignore
      gl.STATIC_DRAW,
    );

    // --- Create Colormap Program and related resources ---
    resources.colormapProgram = createColormapProgram(gl);
    resources.colormapUniforms = {
      uLum: gl.getUniformLocation(resources.colormapProgram, "uLum"),
      uColorMap: gl.getUniformLocation(resources.colormapProgram, "uColorMap"),
      uOverlayAlpha: gl.getUniformLocation(resources.colormapProgram, "uOverlayAlpha"),
    };
    resources.cmapVao = gl.createVertexArray();
    if (!resources.cmapVao) throw new Error("Failed to create colormap VAO");
    gl.bindVertexArray(resources.cmapVao);
    resources.cmapVbo = gl.createBuffer();
    if (!resources.cmapVbo) throw new Error("Failed to create colormap VBO");
    gl.bindBuffer(gl.ARRAY_BUFFER, resources.cmapVbo);
    const cmapQuadVertices = new Float32Array([
          // positions // texCoords
          -1.0,  1.0, 0.0, 1.0, // top-left
          -1.0, -1.0, 0.0, 0.0, // bottom-left
           1.0,  1.0, 1.0, 1.0, // top-right
           1.0, -1.0, 1.0, 0.0, // bottom-right
        ]); // prettier-ignore
    gl.bufferData(gl.ARRAY_BUFFER, cmapQuadVertices, gl.STATIC_DRAW);
    const aPosCmapLoc = gl.getAttribLocation(resources.colormapProgram, "aPos");
    const aUVCmapLoc = gl.getAttribLocation(resources.colormapProgram, "aUV");
    gl.enableVertexAttribArray(aPosCmapLoc);
    gl.vertexAttribPointer(aPosCmapLoc, 2, gl.FLOAT, false, 16, 0); // 2 floats position, 4*4=16 bytes stride, 0 offset
    gl.enableVertexAttribArray(aUVCmapLoc);
    gl.vertexAttribPointer(aUVCmapLoc, 2, gl.FLOAT, false, 16, 8); // 2 floats UV, 16 bytes stride, 8 bytes offset
    gl.bindVertexArray(null); // Unbind VAO
    gl.bindBuffer(gl.ARRAY_BUFFER, null); // Unbind VBO

    // --- Setup Vertex Attributes for Resize Programs ---
    gl.bindBuffer(gl.ARRAY_BUFFER, resources.resizePosBuffer);
    // Lanczos (uses VS_RESIZER)
    gl.enableVertexAttribArray(aPosResizeLoc);
    gl.enableVertexAttribArray(aUVResizeLoc);
    gl.vertexAttribPointer(aPosResizeLoc, 2, gl.FLOAT, false, 16, 0); // Stride 16, Offset 0
    gl.vertexAttribPointer(aUVResizeLoc, 2, gl.FLOAT, false, 16, 8); // Stride 16, Offset 8
    // Bilinear (uses VS_BILINEAR_RESIZER)
    gl.enableVertexAttribArray(aPosBilinearLoc);
    gl.enableVertexAttribArray(aUVBilinearLoc);
    gl.vertexAttribPointer(aPosBilinearLoc, 2, gl.FLOAT, false, 16, 0); // Stride 16, Offset 0
    gl.vertexAttribPointer(aUVBilinearLoc, 2, gl.FLOAT, false, 16, 8); // Stride 16, Offset 8

    gl.bindBuffer(gl.ARRAY_BUFFER, null); // Unbind buffer

    // Initialize texture cache
    resources.textureCache = {
      texMid: null,
      texResized: null,
      fbMid: null,
      fbo: null,
      cmapTex: null,
      lastMidSize: null,
      lastResizedSize: null,
    };

    // Check if all required resources were created
    if (
      !resources.resizeProgram ||
      !resources.bilinearResizeProgram ||
      !resources.colormapProgram ||
      !resources.resizeUniforms ||
      !resources.bilinearResizeUniforms ||
      !resources.colormapUniforms ||
      !resources.resizePosBuffer ||
      !resources.cmapVao ||
      !resources.cmapVbo ||
      !resources.textureCache
    ) {
      throw new Error("Failed to initialize all WebGL resources.");
    }

    numWebGLResources += 1;

    return resources as WebGLResources;
  } catch (error) {
    console.error("Error initializing WebGL resources:", error);
    // Clean up partially created resources
    if (gl) {
      if (resources.resizeProgram) gl.deleteProgram(resources.resizeProgram);
      if (resources.bilinearResizeProgram) gl.deleteProgram(resources.bilinearResizeProgram);
      if (resources.colormapProgram) gl.deleteProgram(resources.colormapProgram);
      if (resources.resizePosBuffer) gl.deleteBuffer(resources.resizePosBuffer);
      if (resources.cmapVao) gl.deleteVertexArray(resources.cmapVao);
      if (resources.cmapVbo) gl.deleteBuffer(resources.cmapVbo);
      // Clean up texture cache
      if (resources.textureCache) {
        if (resources.textureCache.texMid) gl.deleteTexture(resources.textureCache.texMid);
        if (resources.textureCache.texResized) gl.deleteTexture(resources.textureCache.texResized);
        if (resources.textureCache.fbMid) gl.deleteFramebuffer(resources.textureCache.fbMid);
        if (resources.textureCache.fbo) gl.deleteFramebuffer(resources.textureCache.fbo);
        if (resources.textureCache.cmapTex) gl.deleteTexture(resources.textureCache.cmapTex);
      }
    }
    return null;
  }
}

// Internal helper function to encapsulate common rendering logic
function renderSpectrogramInternal(
  webglResources: WebGLResources,
  mipmap: Mipmap,
  srcLeft: number,
  srcTop: number,
  srcW: number,
  srcH: number,
  dstW: number,
  dstH: number,
  blend: number,
  resizeProgram: WebGLProgram,
  resizeUniforms: WebGLResources["resizeUniforms"] | WebGLResources["bilinearResizeUniforms"],
  isBilinear: boolean,
) {
  const {gl, colormapProgram, colormapUniforms, resizePosBuffer, cmapVao, textureCache} =
    webglResources;

  // --- Initial checks and clearing (same as before) ---
  if (blend <= 0) {
    gl.viewport(0, 0, gl.drawingBufferWidth, gl.drawingBufferHeight);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);

    if (dstW > 0 && dstH > 0) {
      gl.enable(gl.SCISSOR_TEST);
      gl.scissor(0, 0, dstW, dstH);
      gl.clearColor(0, 0, 0, 1);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.disable(gl.SCISSOR_TEST);
    }
    return;
  }

  // Vertical texture coordinates parameters
  const vTexOffset = srcTop / mipmap.height;
  const vTexScale = srcH / mipmap.height;

  let texSrc: WebGLTexture | null = null;

  try {
    // Use the provided Resize Program
    gl.useProgram(resizeProgram);

    // Bind the shared position/UV buffer for resize passes
    gl.bindBuffer(gl.ARRAY_BUFFER, resizePosBuffer);

    // --- Texture and Framebuffer Setup ---
    const data = mipmap.arr;
    texSrc = createTexture(gl, mipmap.width, mipmap.height, data, gl.R32F);

    // Check if we need to recreate intermediate texture
    if (
      !textureCache.texMid ||
      !textureCache.lastMidSize ||
      textureCache.lastMidSize.width !== dstW ||
      textureCache.lastMidSize.height !== srcH
    ) {
      if (textureCache.texMid) gl.deleteTexture(textureCache.texMid);
      if (textureCache.fbMid) gl.deleteFramebuffer(textureCache.fbMid);
      textureCache.texMid = createTexture(gl, dstW, srcH, null, gl.R32F);
      textureCache.fbMid = gl.createFramebuffer();
      textureCache.lastMidSize = {width: dstW, height: srcH};
    }

    // Check if we need to recreate final texture
    if (
      !textureCache.texResized ||
      !textureCache.lastResizedSize ||
      textureCache.lastResizedSize.width !== dstW ||
      textureCache.lastResizedSize.height !== dstH
    ) {
      if (textureCache.texResized) gl.deleteTexture(textureCache.texResized);
      if (textureCache.fbo) gl.deleteFramebuffer(textureCache.fbo);
      textureCache.texResized = createTexture(gl, dstW, dstH, null, gl.R32F);
      textureCache.fbo = gl.createFramebuffer();
      textureCache.lastResizedSize = {width: dstW, height: dstH};
    }

    // Set texture unit 0 for the sampler
    gl.uniform1i(resizeUniforms.uTex, 0);
    gl.activeTexture(gl.TEXTURE0);

    // --- Pass-1 (horizontal resize + vertical crop setup) ---
    const scaleX = dstW / srcW;
    gl.uniform2f(resizeUniforms.uStep, 1 / mipmap.width, 0);
    gl.uniform1f(resizeUniforms.uTexOffset, srcLeft / mipmap.width);
    gl.uniform1f(resizeUniforms.uTexScale, srcW / mipmap.width);
    gl.uniform1f(resizeUniforms.uTexOffsetY, vTexOffset);
    gl.uniform1f(resizeUniforms.uTexScaleY, vTexScale);
    if (!isBilinear && "uScale" in resizeUniforms) {
      gl.uniform1f(resizeUniforms.uScale, scaleX);
    }

    gl.bindTexture(gl.TEXTURE_2D, texSrc);
    gl.bindFramebuffer(gl.FRAMEBUFFER, textureCache.fbMid);
    gl.framebufferTexture2D(
      gl.FRAMEBUFFER,
      gl.COLOR_ATTACHMENT0,
      gl.TEXTURE_2D,
      textureCache.texMid,
      0,
    );
    if (gl.checkFramebufferStatus(gl.FRAMEBUFFER) !== gl.FRAMEBUFFER_COMPLETE) {
      throw new Error("Framebuffer 'fbMid' incomplete");
    }
    gl.viewport(0, 0, dstW, srcH);
    gl.drawArrays(gl.TRIANGLES, 0, 6);

    // --- Pass-2 (vertical resize) ---
    const scaleY = dstH / srcH;
    gl.uniform2f(resizeUniforms.uStep, 0, 1 / srcH);
    gl.uniform1f(resizeUniforms.uTexOffset, 0.0);
    gl.uniform1f(resizeUniforms.uTexScale, 1.0);
    gl.uniform1f(resizeUniforms.uTexOffsetY, 0.0);
    gl.uniform1f(resizeUniforms.uTexScaleY, 1.0);
    if (!isBilinear && "uScale" in resizeUniforms) {
      gl.uniform1f(resizeUniforms.uScale, scaleY);
    }

    gl.bindTexture(gl.TEXTURE_2D, textureCache.texMid);
    gl.bindFramebuffer(gl.FRAMEBUFFER, textureCache.fbo);
    gl.framebufferTexture2D(
      gl.FRAMEBUFFER,
      gl.COLOR_ATTACHMENT0,
      gl.TEXTURE_2D,
      textureCache.texResized,
      0,
    );
    if (gl.checkFramebufferStatus(gl.FRAMEBUFFER) !== gl.FRAMEBUFFER_COMPLETE) {
      throw new Error("Framebuffer 'fbo' incomplete");
    }
    gl.viewport(0, 0, dstW, dstH);
    gl.drawArrays(gl.TRIANGLES, 0, 6);

    // --- Pass-3 Colormap Application ---
    gl.useProgram(colormapProgram);
    gl.bindVertexArray(cmapVao);

    // Create or reuse colormap texture
    if (!textureCache.cmapTex) {
      textureCache.cmapTex = createCmapTexture(gl);
    }

    const overlayAlpha = blend < 0.5 ? Math.max(0.0, 1.0 - 2.0 * blend) : 0.0;

    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, textureCache.texResized);
    gl.uniform1i(colormapUniforms.uLum, 0);

    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, textureCache.cmapTex);
    gl.uniform1i(colormapUniforms.uColorMap, 1);

    gl.uniform1f(colormapUniforms.uOverlayAlpha, overlayAlpha);

    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.viewport(0, 0, dstW, dstH);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);

    const error = gl.getError();
    const errorType = isBilinear ? "bilinear" : "lanczos";
    if (error !== gl.NO_ERROR) console.error(`WebGL Error after ${errorType} draw:`, error);
  } catch (error) {
    console.error(`Error during WebGL ${isBilinear ? "bilinear" : "lanczos"} draw:`, error);
  } finally {
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.bindTexture(gl.TEXTURE_2D, null);
    gl.bindVertexArray(null);
    gl.bindBuffer(gl.ARRAY_BUFFER, null);

    if (texSrc) gl.deleteTexture(texSrc);
  }
}

export function renderSpectrogram(
  webglResources: WebGLResources,
  mipmap: Mipmap,
  srcLeft: number,
  srcTop: number,
  srcW: number,
  srcH: number,
  dstW: number,
  dstH: number,
  blend: number,
  isBilinear: boolean,
) {
  renderSpectrogramInternal(
    webglResources,
    mipmap,
    srcLeft,
    srcTop,
    srcW,
    srcH,
    dstW,
    dstH,
    blend,
    isBilinear ? webglResources.bilinearResizeProgram : webglResources.resizeProgram,
    isBilinear ? webglResources.bilinearResizeUniforms : webglResources.resizeUniforms,
    isBilinear,
  );
}

export function cleanupWebGLResources(resources: WebGLResources) {
  const {
    gl,
    resizeProgram,
    bilinearResizeProgram,
    colormapProgram,
    resizePosBuffer,
    cmapVao,
    cmapVbo,
    textureCache,
  } = resources;
  gl.deleteProgram(resizeProgram);
  gl.deleteProgram(bilinearResizeProgram);
  gl.deleteProgram(colormapProgram);
  gl.deleteBuffer(resizePosBuffer);
  gl.deleteVertexArray(cmapVao);
  gl.deleteBuffer(cmapVbo);

  // Clean up texture cache
  if (textureCache) {
    if (textureCache.texMid) gl.deleteTexture(textureCache.texMid);
    if (textureCache.texResized) gl.deleteTexture(textureCache.texResized);
    if (textureCache.fbMid) gl.deleteFramebuffer(textureCache.fbMid);
    if (textureCache.fbo) gl.deleteFramebuffer(textureCache.fbo);
    if (textureCache.cmapTex) gl.deleteTexture(textureCache.cmapTex);
  }

  // Attempt to lose context gracefully
  const loseCtxExt = gl.getExtension("WEBGL_lose_context");
  if (loseCtxExt) {
    loseCtxExt.loseContext();
  }

  numWebGLResources -= 1;
}
