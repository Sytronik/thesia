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
import {freqHzToPos} from "backend";
import styles from "./ImgCanvas.module.scss";
import BackendAPI, {Spectrogram} from "../api";
import {
  createTexture,
  createCmapTexture,
  cleanupWebGLResources,
  WebGLResources,
  createProgram,
  VS_RESIZER,
  FS_RESIZER,
  VS_COLORMAP,
  FS_COLORMAP,
} from "../lib/webgl-helpers";

type ImgCanvasProps = {
  spectrogram: Spectrogram | null;
  width: number;
  height: number;
  startSec: number;
  pxPerSec: number;
  trackSec: number;
  maxTrackSec: number;
  hzRange: [number, number];
  maxTrackHz: number;
};

type ImgTooltipInfo = {pos: number[]; lines: string[]};

const calcTooltipPos = (e: React.MouseEvent) => {
  return [e.clientX + 0, e.clientY + 15];
};

const ImgCanvas = forwardRef((props: ImgCanvasProps, ref) => {
  const {
    width,
    height,
    startSec,
    pxPerSec,
    trackSec,
    maxTrackSec,
    spectrogram,
    hzRange,
    maxTrackHz,
  } = props;
  const devicePixelRatio = useContext(DevicePixelRatioContext);

  const canvasElem = useRef<HTMLCanvasElement | null>(null);
  // Combine WebGL resources into a single ref
  const webglResourcesRef = useRef<WebGLResources | null>(null);
  const lastTimestampRef = useRef<number>(0);

  const loadingElem = useRef<HTMLDivElement>(null);
  const tooltipElem = useRef<HTMLSpanElement>(null);
  const [initTooltipInfo, setInitTooltipInfo] = useState<ImgTooltipInfo | null>(null);

  const showLoading = useEvent(() => {
    if (loadingElem.current) loadingElem.current.style.display = "block";
  });

  const getBoundingClientRect = useEvent(() => {
    return canvasElem.current?.getBoundingClientRect() ?? new DOMRect();
  });

  const imperativeInstanceRef = useRef<ImgCanvasHandleElement>({
    showLoading,
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
    const time = Math.min(Math.max(startSec + x / pxPerSec, 0), maxTrackSec);
    const timeStr = time.toFixed(6).slice(0, -3);
    const hz = BackendAPI.freqPosToHz(y, height, hzRange);
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
    if (!spectrogram) return;

    if (loadingElem.current) loadingElem.current.style.display = "none";
  }, [spectrogram]);

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
      desynchronized: true,
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
      const resizeProgram = createProgram(gl, VS_RESIZER, FS_RESIZER);
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
      const colormapProgram = createProgram(gl, VS_COLORMAP, FS_COLORMAP);
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
    () => {
      const resources = webglResourcesRef.current;
      // Ensure WebGL resources are ready
      if (
        !canvasElem.current ||
        !resources ||
        !resources.resizeUniforms.uTex ||
        !resources.resizeUniforms.uScale ||
        !resources.resizeUniforms.uStep ||
        !resources.resizeUniforms.uTexOffset ||
        !resources.resizeUniforms.uTexScale ||
        !resources.resizeUniforms.uTexOffsetY ||
        !resources.resizeUniforms.uTexScaleY
      ) {
        // Optionally clear canvas if resources aren't ready
        // if(resources?.gl) { resources.gl.clearColor(0, 0, 0, 0); resources.gl.clear(resources.gl.COLOR_BUFFER_BIT); }
        return;
      }

      if (loadingElem.current) loadingElem.current.style.display = "none";

      const {gl, resizeProgram, colormapProgram, resizeUniforms, resizePosBuffer, cmapVao} =
        resources;

      // Check if img and img.data are valid before proceeding
      if (!spectrogram || startSec > trackSec || hzRange[0] >= hzRange[1]) {
        gl.clearColor(0, 0, 0, 0); // Clear to transparent black
        gl.clear(gl.COLOR_BUFFER_BIT);
        return;
      }

      // widths
      const srcPxPerSec = spectrogram.width / trackSec;
      const dstLengthSec = width / pxPerSec;
      const srcLeft = startSec * srcPxPerSec;
      let srcW = dstLengthSec * srcPxPerSec;
      let dstW = width * devicePixelRatio;
      if (startSec + dstLengthSec > trackSec) {
        srcW = spectrogram.width - srcLeft;
        dstW = (trackSec - startSec) * pxPerSec * devicePixelRatio;
      }
      srcW = Math.max(0.5, srcW);
      dstW = Math.max(0.5, dstW);

      // heights
      const srcTop =
        spectrogram.height - freqHzToPos(hzRange[0], spectrogram.height, [0, maxTrackHz]);
      const srcBottom =
        spectrogram.height -
        freqHzToPos(Math.min(hzRange[1], maxTrackHz), spectrogram.height, [0, maxTrackHz]);
      const srcH = srcBottom - srcTop;
      const dstH = Math.max(1, Math.floor(height * devicePixelRatio));

      if (srcW <= 0 || srcH <= 0 || dstW <= 0 || dstH <= 0) {
        console.error("Invalid dimensions for textures:", {srcW, srcH, dstW, dstH});
        return; // Skip rendering
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
        gl.clearColor(0, 0, 0, 0);
        gl.clear(gl.COLOR_BUFFER_BIT);
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
    [
      spectrogram,
      startSec,
      trackSec,
      width,
      pxPerSec,
      devicePixelRatio,
      hzRange,
      maxTrackHz,
      height,
    ], // Keep dependencies that affect rendering dimensions/data
  );

  // Use a ref to store the latest draw function
  const drawRef = useRef(draw);
  useEffect(() => {
    drawRef.current = draw;
    // Request a redraw only when the draw function or its dependencies change
    const animationFrameId = requestAnimationFrame((timestamp) => {
      if (timestamp === lastTimestampRef.current) return;
      lastTimestampRef.current = timestamp;
      // Ensure drawRef.current exists and call it
      if (drawRef.current) {
        drawRef.current();
      }
    });

    // Cleanup function to cancel the frame if the component unmounts
    // or if dependencies change again before the frame executes
    return () => cancelAnimationFrame(animationFrameId);
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
