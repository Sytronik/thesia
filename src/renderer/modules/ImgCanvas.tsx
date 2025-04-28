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
  canvasIsFit: boolean;
  bmpBuffer: Buffer | null;
};

type ImgTooltipInfo = {pos: number[]; lines: string[]};

const calcTooltipPos = (e: React.MouseEvent) => {
  return [e.clientX + 0, e.clientY + 15];
};

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

in  vec2 vTex[9];
out vec4 fragColor;

/* Lanczos-3 weights (normalised) */
const float w[5] = float[5](
    0.38026, 0.27667, 0.08074, -0.02612, -0.02143);

void main() {
    /* centre tap */
    vec4 c = texture(uTex, vTex[4]) * w[0];

    /* symmetric wing taps */
    for (int i = 1; i <= 4; ++i) {
        vec4 t = texture(uTex, vTex[4 - i]) +
                 texture(uTex, vTex[4 + i]);
        c += t * w[i];
    }
    fragColor = c;
}`;

/* ---------- 2.  WebGL helpers ---------- */
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
  data: ImageBitmap | null = null,
): WebGLTexture {
  const t = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, t);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  if (data) {
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, data);
  } else {
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
  }
  return t;
}

const ImgCanvas = forwardRef((props: ImgCanvasProps, ref) => {
  const {width, height, maxTrackSec, canvasIsFit, bmpBuffer} = props;
  const devicePixelRatio = useContext(DevicePixelRatioContext);

  const canvasElem = useRef<HTMLCanvasElement | null>(null);
  const glRef = useRef<WebGL2RenderingContext | null>(null);
  const programRef = useRef<WebGLProgram | null>(null);
  const uniformsRef = useRef<{
    uStep: WebGLUniformLocation | null;
    uTex: WebGLUniformLocation | null;
  }>({uStep: null, uTex: null});
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
  const [bitmap, setBitmap] = useState<ImageBitmap | null>(null);

  useEffect(() => {
    if (!bmpBuffer) {
      setBitmap(null);
      return;
    }
    if (loadingElem.current) loadingElem.current.style.display = "none";

    const bmpData = new Uint8Array(bmpBuffer);
    const bmpBlob = new Blob([bmpData.buffer], {type: "image/bmp"});
    createImageBitmap(bmpBlob, {imageOrientation: "flipY"})
      .then((bmp) => {
        setBitmap(bmp);
      })
      .catch(() => {});
  }, [bmpBuffer]);

  const canvasElemCallback = useCallback((elem: HTMLCanvasElement) => {
    canvasElem.current = elem;
    if (!canvasElem.current) {
      glRef.current = null;
      programRef.current = null;
      uniformsRef.current = {uStep: null, uTex: null};
      return;
    }
    const gl = canvasElem.current.getContext("webgl2", {
      antialias: false,
      depth: false,
      preserveDrawingBuffer: true, // Prevent flickering by preserving the buffer
    });
    if (!gl) {
      glRef.current = null;
      programRef.current = null;
      uniformsRef.current = {uStep: null, uTex: null};
      return;
    }

    glRef.current = gl;
    const prog = createProgram(gl, VERT_SRC, FRAG_SRC);
    gl.useProgram(prog);

    /* 3.2  full-screen quad */
    const posBuf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, posBuf);
    gl.bufferData(
      gl.ARRAY_BUFFER,
      new Float32Array([
        -1, -1, 0, 0, 1, -1, 1, 0, -1, 1, 0, 1, -1, 1, 0, 1, 1, -1, 1, 0, 1, 1, 1, 1,
      ]),
      gl.STATIC_DRAW,
    );

    const aPos = gl.getAttribLocation(prog, "aPosition");
    const aUV = gl.getAttribLocation(prog, "aTexCoord");
    gl.enableVertexAttribArray(aPos);
    gl.enableVertexAttribArray(aUV);
    gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 16, 0);
    gl.vertexAttribPointer(aUV, 2, gl.FLOAT, false, 16, 8);

    // Store program and uniform locations
    programRef.current = prog;
    uniformsRef.current = {
      uStep: gl.getUniformLocation(prog, "uStep"),
      uTex: gl.getUniformLocation(prog, "uTex"),
    };
  }, []);

  const draw = useCallback(
    (timestamp: number) => {
      if (timestamp === lastTimestampRef.current) return;
      lastTimestampRef.current = timestamp;
      if (!canvasElem.current || !glRef.current || !programRef.current || !uniformsRef.current)
        return;
      if (loadingElem.current) loadingElem.current.style.display = "none";
      if (!bitmap) {
        glRef.current.clearColor(0, 0, 0, 0);
        glRef.current.clear(glRef.current.COLOR_BUFFER_BIT);
        return;
      }

      // Use stored program
      glRef.current.useProgram(programRef.current);

      /* 3.3  textures & FBO */
      const srcW = bitmap.width;
      const srcH = bitmap.height;
      const dstW = width * devicePixelRatio;
      const dstH = height * devicePixelRatio;

      // upload source
      const texSrc = createTexture(glRef.current, srcW, srcH, bitmap);

      // intermediate texture at final width, source height
      const texMid = createTexture(glRef.current, dstW, srcH);
      const fbo = glRef.current.createFramebuffer();

      glRef.current.uniform1i(uniformsRef.current.uTex, 0); // sampler = TEXTURE0

      /* 3.4  pass-1 (horizontal) */
      glRef.current.activeTexture(glRef.current.TEXTURE0);
      glRef.current.bindTexture(glRef.current.TEXTURE_2D, texSrc);
      glRef.current.bindFramebuffer(glRef.current.FRAMEBUFFER, fbo);
      glRef.current.framebufferTexture2D(
        glRef.current.FRAMEBUFFER,
        glRef.current.COLOR_ATTACHMENT0,
        glRef.current.TEXTURE_2D,
        texMid,
        0,
      );
      glRef.current.viewport(0, 0, dstW, srcH);
      glRef.current.uniform2f(uniformsRef.current.uStep, 1 / srcW, 0);
      glRef.current.drawArrays(glRef.current.TRIANGLES, 0, 6);

      /* 3.5  pass-2 (vertical) */
      glRef.current.bindTexture(glRef.current.TEXTURE_2D, texMid);
      glRef.current.bindFramebuffer(glRef.current.FRAMEBUFFER, null); // canvas
      glRef.current.viewport(0, 0, dstW, dstH);
      glRef.current.uniform2f(uniformsRef.current.uStep, 0, 1 / srcH);
      glRef.current.drawArrays(glRef.current.TRIANGLES, 0, 6);

      // Clean up
      glRef.current.deleteTexture(texSrc);
      glRef.current.deleteTexture(texMid);
      glRef.current.deleteFramebuffer(fbo);
    },
    [width, height, bitmap, devicePixelRatio],
  );

  useEffect(() => {
    requestAnimationFrame(draw);
  }, [draw]);

  return (
    <div
      className={styles.imgCanvasWrapper}
      /* this is needed for consistent layout
         because changing width of canvas elem can occur in different time (in draw function) */
      style={{width, height}}
    >
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
        width={width * devicePixelRatio}
        height={height * devicePixelRatio}
      />
    </div>
  );
});
ImgCanvas.displayName = "ImgCanvas";

export default React.memo(ImgCanvas);
