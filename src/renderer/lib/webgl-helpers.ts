import {COLORMAP_RGBA} from "../prototypes/constants/colors";

export const VS_RESIZER = `#version 300 es
precision highp float;

/* locations 0 & 1 match the calls to gl.vertexAttribPointer(...) */
layout(location = 0) in  vec4 aPosition;
layout(location = 1) in  vec2 aTexCoord;

uniform vec2 uStep;          // (1/srcW,0)  or  (0,1/srcH)

out vec2 vTex[9];            // centre ± 4 offsets

void main() {
    gl_Position = aPosition;

    /* centre sample */
    vTex[4] = aTexCoord;

    /* ±1 … ±4 samples */
    for (int i = 1; i <= 4; ++i) {
        vec2 o = float(i) * uStep;
        vTex[4 - i] = aTexCoord - o;
        vTex[4 + i] = aTexCoord + o;
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
in  vec2 vUV;
out vec4 fragColor;

void main() {
    float l  = texture(uLum, vUV).r;           // 0-to-1 luminance
    vec4  c  = texture(uColorMap, vec2(l, .5)); // lookup
    fragColor = vec4(c.rgb, 1.0);               // solid alpha
}`;

function createShader(gl: WebGL2RenderingContext, type: number, src: string): WebGLShader {
  const s = gl.createShader(type);
  if (!s) throw new Error("Failed to create shader");
  gl.shaderSource(s, src);
  gl.compileShader(s);
  if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw gl.getShaderInfoLog(s);
  return s;
}

export function createProgram(
  gl: WebGL2RenderingContext,
  vsSrc: string,
  fsSrc: string,
): WebGLProgram {
  const p = gl.createProgram();
  gl.attachShader(p, createShader(gl, gl.VERTEX_SHADER, vsSrc));
  gl.attachShader(p, createShader(gl, gl.FRAGMENT_SHADER, fsSrc));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw gl.getProgramInfoLog(p);
  return p;
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
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 256, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, COLORMAP_RGBA);
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
  };
  resizePosBuffer: WebGLBuffer | null; // Buffer for vertex/UV data used in resize passes
  cmapVao: WebGLVertexArrayObject | null; // VAO for the colormap pass fullscreen quad
  cmapVbo: WebGLBuffer | null; // VBO for the colormap pass fullscreen quad
};

export function cleanupWebGLResources(resources: WebGLResources) {
  const {gl, resizeProgram, colormapProgram, resizePosBuffer, cmapVao, cmapVbo} = resources;
  gl.deleteProgram(resizeProgram);
  gl.deleteProgram(colormapProgram);
  gl.deleteBuffer(resizePosBuffer);
  gl.deleteVertexArray(cmapVao);
  gl.deleteBuffer(cmapVbo);
}
