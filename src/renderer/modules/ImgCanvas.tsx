import React, {
  forwardRef,
  useRef,
  useImperativeHandle,
  useState,
  useContext,
  useMemo,
  useEffect,
  useCallback,
} from "react";
import useEvent from "react-use-event-hook";
import {throttle} from "throttle-debounce";
import {DevicePixelRatioContext} from "renderer/contexts";
import styles from "./ImgCanvas.module.scss";
import BackendAPI from "../api";

type ImgCanvasProps = {
  width: number;
  height: number;
  maxTrackSec: number;
  imgInfo: {buf: Buffer; width: number; height: number} | null;
};

type ImgTooltipInfo = {pos: number[]; lines: string[]};

const calcTooltipPos = (e: React.MouseEvent) => {
  return [e.clientX + 0, e.clientY + 15];
};

const COLORMAP_RGB = [
  [0, 0, 4], [1, 0, 5], [1, 1, 6], [1, 1, 8], [2, 1, 10], [2, 2, 12], [2, 2, 14], [3, 2, 16], [4, 3, 18], [4, 3, 21],
  [5, 4, 23], [6, 4, 25], [7, 5, 27], [8, 6, 29], [9, 6, 32], [10, 7, 34], [11, 7, 36], [12, 8, 38], [13, 8, 41], [14, 9, 43],
  [16, 9, 45], [17, 10, 48], [18, 10, 50], [20, 11, 53], [21, 11, 55], [22, 11, 58], [24, 12, 60], [25, 12, 62], [27, 12, 65], [28, 12, 67],
  [30, 12, 70], [31, 12, 72], [33, 12, 74], [35, 12, 77], [36, 12, 79], [38, 12, 81], [40, 11, 83], [42, 11, 85], [43, 11, 87], [45, 11, 89],
  [47, 10, 91], [49, 10, 93], [51, 10, 94], [52, 10, 96], [54, 9, 97], [56, 9, 98], [58, 9, 99], [59, 9, 100], [61, 9, 101], [63, 9, 102],
  [64, 10, 103], [66, 10, 104], [68, 10, 105], [69, 10, 105], [71, 11, 106], [73, 11, 107], [74, 12, 107], [76, 12, 108], [78, 13, 108], [79, 13, 108],
  [81, 14, 109], [83, 14, 109], [84, 15, 109], [86, 15, 110], [87, 16, 110], [89, 17, 110], [91, 17, 110], [92, 18, 110], [94, 18, 111], [95, 19, 111],
  [97, 20, 111], [99, 20, 111], [100, 21, 111], [102, 21, 111], [103, 22, 111], [105, 23, 111], [107, 23, 111], [108, 24, 111], [110, 24, 111], [111, 25, 111],
  [113, 25, 110], [115, 26, 110], [116, 27, 110], [118, 27, 110], [119, 28, 110], [121, 28, 110], [123, 29, 109], [124, 29, 109], [126, 30, 109], [127, 31, 109],
  [129, 31, 108], [130, 32, 108], [132, 32, 108], [134, 33, 107], [135, 33, 107], [137, 34, 107], [138, 34, 106], [140, 35, 106], [142, 36, 105], [143, 36, 105],
  [145, 37, 105], [146, 37, 104], [148, 38, 104], [150, 38, 103], [151, 39, 102], [153, 40, 102], [154, 40, 101], [156, 41, 101], [158, 41, 100], [159, 42, 100],
  [161, 42, 99], [162, 43, 98], [164, 43, 97], [165, 44, 96], [167, 45, 95], [169, 45, 94], [170, 46, 93], [172, 46, 92], [173, 47, 91], [175, 48, 90],
  [176, 48, 89], [178, 49, 88], [179, 49, 87], [181, 50, 86], [182, 51, 85], [184, 51, 84], [185, 52, 83], [187, 53, 82], [188, 54, 81], [190, 54, 80],
  [191, 55, 79], [193, 56, 78], [194, 57, 77], [196, 58, 76], [197, 59, 75], [198, 60, 74], [200, 60, 72], [201, 61, 71], [203, 62, 70], [204, 63, 69],
  [205, 64, 68], [207, 65, 67], [208, 66, 66], [209, 67, 65], [211, 68, 64], [212, 69, 62], [213, 70, 61], [214, 72, 60], [216, 73, 59], [217, 74, 58],
  [218, 75, 57], [219, 76, 56], [220, 77, 54], [221, 79, 53], [223, 80, 52], [224, 81, 51], [225, 82, 50], [226, 84, 48], [227, 85, 47], [228, 86, 46],
  [229, 88, 45], [230, 89, 43], [231, 90, 42], [232, 92, 41], [233, 93, 40], [234, 95, 38], [235, 96, 37], [235, 98, 36], [236, 99, 35], [237, 101, 33],
  [238, 102, 32], [239, 104, 31], [240, 105, 30], [240, 107, 28], [241, 109, 27], [242, 110, 26], [242, 112, 24], [243, 113, 23], [244, 115, 22], [244, 117, 20],
  [245, 118, 19], [246, 120, 18], [246, 122, 16], [247, 123, 15], [247, 125, 14], [248, 127, 12], [248, 129, 11], [249, 130, 10], [249, 132, 9], [249, 134, 7],
  [250, 136, 6], [250, 137, 6], [250, 139, 6], [251, 141, 6], [251, 143, 6], [251, 145, 6], [252, 146, 6], [252, 148, 6], [252, 150, 6], [252, 152, 6],
  [253, 154, 7], [253, 156, 7], [253, 158, 7], [253, 160, 7], [253, 161, 7], [253, 163, 7], [253, 165, 7], [253, 167, 7], [253, 169, 7], [253, 171, 7],
  [253, 173, 7], [253, 175, 7], [253, 177, 7], [253, 179, 7], [252, 181, 7], [252, 183, 7], [252, 185, 7], [252, 186, 7], [252, 188, 7], [251, 190, 7],
  [251, 192, 7], [251, 194, 7], [251, 196, 7], [250, 198, 7], [250, 200, 7], [250, 202, 7], [249, 204, 7], [249, 206, 7], [248, 208, 7], [248, 210, 7],
  [247, 212, 7], [247, 214, 7], [246, 216, 7], [246, 218, 7], [245, 220, 7], [245, 222, 7], [244, 224, 7], [244, 226, 7], [244, 228, 7], [243, 229, 7],
  [243, 231, 7], [243, 233, 7], [242, 235, 7], [242, 237, 7], [242, 238, 7], [243, 240, 7], [243, 241, 7], [244, 243, 7], [244, 244, 7], [245, 246, 7],
  [246, 247, 7], [247, 249, 7], [249, 250, 7], [250, 251, 7], [251, 253, 7], [253, 254, 7], [253, 255, 7], [253, 255, 7], [253, 255, 7], [253, 255, 7], [255, 255, 255]
]; // prettier-ignore

const COLORMAP_RGBA = new Uint8Array(256 * 4);
COLORMAP_RGB.forEach(([r, g, b], i) => {
  const o = i * 4;
  COLORMAP_RGBA[o] = r;
  COLORMAP_RGBA[o + 1] = g;
  COLORMAP_RGBA[o + 2] = b;
  COLORMAP_RGBA[o + 3] = 255; // opaque
});

/* ---------- 1.  GLSL sources ---------- */
const VERT_SRC = `#version 300 es
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

const FRAG_SRC = `#version 300 es
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

const VERT_SRC_CMAP = `#version 300 es
layout(location=0) in vec2 aPos;   // fullscreen quad
layout(location=1) in vec2 aUV;
out vec2 vUV;
void main() {
    vUV = aUV;
    gl_Position = vec4(aPos, 0.0, 1.0);
}`;

const FRAG_SRC_CMAP = `#version 300 es
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

function createProgram(gl: WebGL2RenderingContext, vsSrc: string, fsSrc: string): WebGLProgram {
  const p = gl.createProgram();
  gl.attachShader(p, createShader(gl, gl.VERTEX_SHADER, vsSrc));
  gl.attachShader(p, createShader(gl, gl.FRAGMENT_SHADER, fsSrc));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw gl.getProgramInfoLog(p);
  return p;
}

function createTexture(
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

function createCmapTexture(gl: WebGL2RenderingContext) {
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

type WebGLResources = {
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

function cleanupWebGLResources(resources: WebGLResources) {
  const {gl, resizeProgram, colormapProgram, resizePosBuffer, cmapVao, cmapVbo} = resources;
  gl.deleteProgram(resizeProgram);
  gl.deleteProgram(colormapProgram);
  gl.deleteBuffer(resizePosBuffer);
  gl.deleteVertexArray(cmapVao);
  gl.deleteBuffer(cmapVbo);
}

const ImgCanvas = forwardRef((props: ImgCanvasProps, ref) => {
  const {width, height, maxTrackSec, imgInfo} = props;
  const devicePixelRatio = useContext(DevicePixelRatioContext);

  const [img, setImg] = useState<{data: Float32Array; width: number; height: number} | null>(null);
  const canvasElem = useRef<HTMLCanvasElement | null>(null);
  // Combine WebGL resources into a single ref
  const webglResourcesRef = useRef<WebGLResources | null>(null);
  const lastTimestampRef = useRef<number>(0);

  const loadingElem = useRef<HTMLDivElement>(null);
  const startSecRef = useRef<number>(0);
  const pxPerSecRef = useRef<number>(1);
  const tooltipElem = useRef<HTMLSpanElement>(null);
  const [initTooltipInfo, setInitTooltipInfo] = useState<ImgTooltipInfo | null>(null);

  const showLoading = useEvent(() => {
    if (loadingElem.current) loadingElem.current.style.display = "block";
  });

  const updateLensParams = useEvent((params: OptionalLensParams) => {
    startSecRef.current = params.startSec ?? startSecRef.current;
    pxPerSecRef.current = params.pxPerSec ?? pxPerSecRef.current;
  });

  const getBoundingClientRect = useEvent(() => {
    return canvasElem.current?.getBoundingClientRect() ?? new DOMRect();
  });

  const imperativeInstanceRef = useRef<ImgCanvasHandleElement>({
    showLoading,
    updateLensParams,
    getBoundingClientRect,
  });
  useImperativeHandle(ref, () => imperativeInstanceRef.current, []);

  const getTooltipLines = useEvent(async (e: React.MouseEvent) => {
    if (!canvasElem.current) return ["sec", "Hz"];
    const x = e.clientX - canvasElem.current.getBoundingClientRect().left;
    const y = Math.min(
      Math.max(e.clientY - canvasElem.current.getBoundingClientRect().top, 0),
      height,
    );
    // TODO: need better formatting (from backend?)
    const time = Math.min(Math.max(startSecRef.current + x / pxPerSecRef.current, 0), maxTrackSec);
    const timeStr = time.toFixed(6).slice(0, -3);
    const hz = await BackendAPI.freqPosToHzOnCurrentRange(y, height);
    const hzStr = hz.toFixed(0);
    return [`${timeStr} sec`, `${hzStr} Hz`];
  });

  const onMouseMove = useMemo(
    () =>
      throttle(1000 / 120, async (e: React.MouseEvent) => {
        if (initTooltipInfo === null || tooltipElem.current === null) return;
        const [left, top] = calcTooltipPos(e);
        tooltipElem.current.style.left = `${left}px`;
        tooltipElem.current.style.top = `${top}px`;
        const lines = await getTooltipLines(e);
        lines.forEach((v, i) => {
          const node = tooltipElem.current?.children.item(i) ?? null;
          if (node) node.innerHTML = v;
        });
      }),
    [getTooltipLines, initTooltipInfo],
  );

  useEffect(() => {
    if (!imgInfo) {
      setImg(null);
      return;
    }
    if (loadingElem.current) loadingElem.current.style.display = "none";

    const {buf, width: bitmapWidth, height: bitmapHeight} = imgInfo;
    const data = new Float32Array(buf.buffer);

    setImg({data, width: bitmapWidth, height: bitmapHeight});
  }, [imgInfo]);

  const canvasElemCallback = useCallback((elem: HTMLCanvasElement | null) => {
    // Cleanup previous resources if the element changes
    if (webglResourcesRef.current?.gl && elem !== canvasElem.current) {
      cleanupWebGLResources(webglResourcesRef.current);
      webglResourcesRef.current = null;
    }

    canvasElem.current = elem;
    if (!canvasElem.current) {
      webglResourcesRef.current = null;
      return;
    }

    const gl = canvasElem.current.getContext("webgl2", {
      antialias: false,
      depth: false,
      preserveDrawingBuffer: true,
    });

    if (!gl) {
      console.error("Failed to get WebGL2 context.");
      webglResourcesRef.current = null;
      return;
    }

    // Check for float buffer support
    const ext = gl.getExtension("EXT_color_buffer_float");
    if (!ext) {
      console.warn(
        "WebGL extension 'EXT_color_buffer_float' not supported. Rendering to float textures might fail.",
      );
    }

    try {
      // --- Create Resize Program and related resources ---
      const resizeProgram = createProgram(gl, VERT_SRC, FRAG_SRC);
      const resizeUniforms = {
        uStep: gl.getUniformLocation(resizeProgram, "uStep"),
        uTex: gl.getUniformLocation(resizeProgram, "uTex"),
        uScale: gl.getUniformLocation(resizeProgram, "uScale"),
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
      const colormapProgram = createProgram(gl, VERT_SRC_CMAP, FRAG_SRC_CMAP);
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
      webglResourcesRef.current = {
        gl,
        resizeProgram,
        colormapProgram,
        resizeUniforms,
        resizePosBuffer,
        cmapVao,
        cmapVbo,
      };

      // Setup vertex attributes for the resize program (can be done once)
      gl.bindBuffer(gl.ARRAY_BUFFER, webglResourcesRef.current.resizePosBuffer);
      gl.enableVertexAttribArray(aPosResizeLoc);
      gl.enableVertexAttribArray(aUVResizeLoc);
      // Stride is 16 bytes (4 floats: PosX, PosY, UVx, UVy), Pos is offset 0, UV is offset 8
      gl.vertexAttribPointer(aPosResizeLoc, 2, gl.FLOAT, false, 16, 0);
      gl.vertexAttribPointer(aUVResizeLoc, 2, gl.FLOAT, false, 16, 8);
      gl.bindBuffer(gl.ARRAY_BUFFER, null); // Unbind buffer
    } catch (error) {
      console.error("Error initializing WebGL resources:", error);
      // Clean up partially created resources if necessary
      if (gl) {
        // Attempt to delete any resources that might have been created before the error
        const currentRes = webglResourcesRef.current;
        if (currentRes) {
          gl.deleteProgram(currentRes.resizeProgram);
          gl.deleteProgram(currentRes.colormapProgram);
          gl.deleteBuffer(currentRes.resizePosBuffer);
          gl.deleteVertexArray(currentRes.cmapVao);
          gl.deleteBuffer(currentRes.cmapVbo);
        } else {
          // If webglResourcesRef wasn't set yet, try deleting based on local vars
          // This requires careful handling as some vars might be undefined if error occurred early
          // Example: if (resizeProgram) gl.deleteProgram(resizeProgram); etc.
        }
      }
      webglResourcesRef.current = null;
    }
  }, []); // Empty dependency array: This setup runs once per canvas element instance.

  const draw = useCallback(
    (timestamp: number) => {
      if (timestamp === lastTimestampRef.current) return;
      lastTimestampRef.current = timestamp;

      const resources = webglResourcesRef.current;
      // Ensure WebGL resources are ready
      if (
        !canvasElem.current ||
        !resources ||
        !resources.resizeUniforms.uTex || // Check essential uniforms too
        !resources.resizeUniforms.uScale ||
        !resources.resizeUniforms.uStep
      ) {
        // Optionally clear canvas if resources aren't ready
        // if(resources?.gl) { resources.gl.clearColor(0, 0, 0, 0); resources.gl.clear(resources.gl.COLOR_BUFFER_BIT); }
        return;
      }

      if (loadingElem.current) loadingElem.current.style.display = "none";

      const {gl, resizeProgram, colormapProgram, resizeUniforms, resizePosBuffer, cmapVao} =
        resources;

      // Check if img and img.data are valid before proceeding
      if (!img || !img.data || img.data.length === 0) {
        gl.clearColor(0, 0, 0, 0); // Clear to transparent black
        gl.clear(gl.COLOR_BUFFER_BIT);
        return;
      }

      const srcW = img.width;
      const srcH = img.height;
      const dstW = Math.max(1, Math.floor(width * devicePixelRatio)); // Ensure positive dims
      const dstH = Math.max(1, Math.floor(height * devicePixelRatio));

      if (srcW <= 0 || srcH <= 0 || dstW <= 0 || dstH <= 0) {
        console.error("Invalid dimensions for textures:", {srcW, srcH, dstW, dstH});
        return; // Skip rendering
      }

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
        texSrc = createTexture(gl, srcW, srcH, img.data, gl.R32F);

        // Create intermediate R32F texture for horizontal pass result
        texMid = createTexture(gl, dstW, srcH, null, gl.R32F);
        fbMid = gl.createFramebuffer();

        // Create final R32F texture for fully resized result
        texResized = createTexture(gl, dstW, dstH, null, gl.R32F);
        fbo = gl.createFramebuffer();

        // Set texture unit 0 for the sampler
        gl.uniform1i(resizeUniforms.uTex, 0);
        gl.activeTexture(gl.TEXTURE0);

        // --- Pass-1 (horizontal resize) ---
        const scaleX = dstW / srcW;
        gl.uniform1f(resizeUniforms.uScale, scaleX);
        gl.uniform2f(resizeUniforms.uStep, 1 / srcW, 0); // Horizontal step
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
        gl.uniform2f(resizeUniforms.uStep, 0, 1 / srcH); // Vertical step
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

        // Setup textures for colormap pass
        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, texResized); // Use the final resized R32F texture
        gl.uniform1i(gl.getUniformLocation(colormapProgram, "uLum"), 0);

        gl.activeTexture(gl.TEXTURE1);
        gl.bindTexture(gl.TEXTURE_2D, cmapTex);
        gl.uniform1i(gl.getUniformLocation(colormapProgram, "uColorMap"), 1);

        // Render to canvas
        gl.bindFramebuffer(gl.FRAMEBUFFER, null);
        gl.viewport(0, 0, dstW, dstH); // Ensure viewport matches canvas destination size
        // Attributes are already set up via cmapVao
        gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4); // Draw the quad using TRIANGLE_STRIP

        // Check for WebGL errors after drawing
        const error = gl.getError();
        if (error !== gl.NO_ERROR) {
          console.error("WebGL Error after draw:", error);
        }
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
    },
    [width, height, img, devicePixelRatio], // Keep dependencies that affect rendering dimensions/data
  );

  useEffect(() => {
    requestAnimationFrame(draw);
  }, [draw]);

  // Cleanup WebGL resources on unmount or when canvas element changes
  useEffect(() => {
    return () => {
      const resources = webglResourcesRef.current;
      if (resources?.gl) {
        cleanupWebGLResources(resources);
      }
      webglResourcesRef.current = null; // Clear the ref
    };
  }, []);

  return (
    <div className={styles.imgCanvasWrapper} style={{width, height}}>
      {initTooltipInfo !== null ? (
        <span
          key="img-canvas-tooltip"
          ref={tooltipElem}
          className={styles.tooltip}
          style={{left: `${initTooltipInfo.pos[0]}px`, top: `${initTooltipInfo.pos[1]}px`}}
        >
          {initTooltipInfo.lines.map((v) => (
            <p key={`img-tooltip-${v.split(" ")[1]}`}>{v}</p>
          ))}
        </span>
      ) : null}
      <div ref={loadingElem} className={styles.loading} style={{display: "none"}} />
      <canvas
        className={styles.ImgCanvas}
        ref={canvasElemCallback}
        style={{width: "100%", height: "100%"}}
        onMouseEnter={async (e) => {
          if (e.buttons !== 0) return;
          setInitTooltipInfo({pos: calcTooltipPos(e), lines: await getTooltipLines(e)});
        }}
        onMouseMove={onMouseMove}
        onMouseLeave={() => setInitTooltipInfo(null)}
        width={Math.max(1, Math.floor(width * devicePixelRatio))}
        height={Math.max(1, Math.floor(height * devicePixelRatio))}
      />
    </div>
  );
});
ImgCanvas.displayName = "ImgCanvas";

export default React.memo(ImgCanvas);
