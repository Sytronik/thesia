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
import {freqHzToPos, WavImage} from "backend";
import styles from "./ImgCanvas.module.scss";
import BackendAPI from "../api";
import {
  createTexture,
  createCmapTexture,
  cleanupWebGLResources,
  WebGLResources,
  createResizeProgram,
  createColormapProgram,
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
  idChStr: string;
  ampRange: [number, number];
  blend: number;
  needRefreshWavImg: boolean;
};

type ImgTooltipInfo = {pos: number[]; lines: string[]};

const calcTooltipPos = (e: React.MouseEvent) => [e.clientX + 0, e.clientY + 15];

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
    idChStr,
    ampRange,
    blend,
    needRefreshWavImg,
  } = props;
  const devicePixelRatio = useContext(DevicePixelRatioContext);
  const wavImageRef = useRef<WavImage | null>(null);

  const specCanvasElem = useRef<HTMLCanvasElement | null>(null);
  const webglResourcesRef = useRef<WebGLResources | null>(null);
  const wavCanvasElem = useRef<HTMLCanvasElement | null>(null);
  const wavCtxRef = useRef<ImageBitmapRenderingContext | null>(null);

  const loadingElem = useRef<HTMLDivElement>(null);
  const tooltipElem = useRef<HTMLSpanElement>(null);
  const [initTooltipInfo, setInitTooltipInfo] = useState<ImgTooltipInfo | null>(null);

  const getBoundingClientRect = useEvent(() => {
    return specCanvasElem.current?.getBoundingClientRect() ?? new DOMRect();
  });

  const imperativeInstanceRef = useRef<ImgCanvasHandleElement>({getBoundingClientRect});
  useImperativeHandle(ref, () => imperativeInstanceRef.current, []);

  const specCanvasElemCallback = useCallback((elem: HTMLCanvasElement | null) => {
    // Cleanup previous resources if the element changes
    if (webglResourcesRef.current?.gl && elem !== specCanvasElem.current) {
      cleanupWebGLResources(webglResourcesRef.current);
      webglResourcesRef.current = null;
    }

    specCanvasElem.current = elem;
    if (!specCanvasElem.current) {
      webglResourcesRef.current = null;
      return;
    }

    const gl = specCanvasElem.current.getContext("webgl2", {
      alpha: true,
      antialias: false,
      depth: false,
      preserveDrawingBuffer: true,
      // desynchronized: true,  // cause flickering when resizing on Windows 10
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
        "WebGL extension 'EXT_color_buffer_float' not supported. " +
          "Rendering to float textures might fail.",
      );
    }

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
      webglResourcesRef.current = {
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

  const wavCanvasElemCallback = useCallback((elem: HTMLCanvasElement | null) => {
    wavCanvasElem.current = elem;

    if (!wavCanvasElem.current) {
      wavCtxRef.current = null;
      return;
    }

    wavCtxRef.current = wavCanvasElem.current.getContext("bitmaprenderer", {alpha: true});

    if (!wavCtxRef.current) {
      console.error("Failed to get bitmaprenderer context.");
      wavCtxRef.current = null;
    }
  }, []);

  const drawSpectrogram = useCallback(() => {
    const resources = webglResourcesRef.current;
    // Ensure WebGL resources are ready
    if (!specCanvasElem.current || !resources) return;

    const {
      gl,
      resizeProgram,
      colormapProgram,
      resizeUniforms,
      colormapUniforms,
      resizePosBuffer,
      cmapVao,
    } = resources;

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
  }, [
    spectrogram,
    startSec,
    trackSec,
    width,
    pxPerSec,
    devicePixelRatio,
    hzRange,
    maxTrackHz,
    height,
    blend,
  ]);

  const drawWav = useCallback(() => {
    if (!wavCanvasElem.current || !wavCtxRef.current || !wavImageRef.current) return;
    const ctx = wavCtxRef.current;
    const imdata = new Uint8ClampedArray(wavImageRef.current.buf);
    wavCanvasElem.current.style.opacity = blend < 0.5 ? "1" : `${Math.min(2 - 2 * blend, 1)}`;
    const img = new ImageData(imdata, wavImageRef.current.width, wavImageRef.current.height);
    createImageBitmap(img)
      .then((bitmap) => ctx.transferFromImageBitmap(bitmap))
      .catch((err) => console.error("Failed to transfer image bitmap:", err));
  }, [blend]);

  // Draw spectrogram
  // Use a ref to store the latest draw function
  const drawSpectrogramRef = useRef(drawSpectrogram);
  const lastSpecTimestampRef = useRef<number>(-1);
  useEffect(() => {
    drawSpectrogramRef.current = drawSpectrogram;
    // Request a redraw only when the draw function or its dependencies change
    const animationFrameId = requestAnimationFrame((timestamp) => {
      if (timestamp === lastSpecTimestampRef.current) return;
      lastSpecTimestampRef.current = timestamp;
      // Ensure drawRef.current exists and call it
      if (drawSpectrogramRef.current) drawSpectrogramRef.current();
    });

    // Cleanup function to cancel the frame if the component unmounts
    // or if dependencies change again before the frame executes
    return () => cancelAnimationFrame(animationFrameId);
  }, [drawSpectrogram]);

  // Draw wav
  const drawWavRef = useRef(drawWav);
  const lastWavTimestampRef = useRef<number>(-1);
  const drawWavOnNextFrame = useEvent((force: boolean = false) => {
    drawWavRef.current = drawWav;
    // Request a redraw only when the draw function or its dependencies change
    const animationFrameId = requestAnimationFrame((timestamp) => {
      if (timestamp === lastWavTimestampRef.current && !force) return;
      lastWavTimestampRef.current = timestamp;
      // Ensure drawRef.current exists and call it
      if (drawWavRef.current) drawWavRef.current();
    });

    // Cleanup function to cancel the frame if the component unmounts
    // or if dependencies change again before the frame executes
    return () => cancelAnimationFrame(animationFrameId);
  });

  const prevGetWavImageRef = useRef<() => void>(() => {});
  const getWavImage = useCallback(() => {
    BackendAPI.getWavImage(idChStr, startSec, pxPerSec, width, height, ampRange, devicePixelRatio)
      .then((wavImage) => {
        wavImageRef.current = wavImage;
        drawWavOnNextFrame(true);
      })
      .catch((err) => {
        console.error("Failed to get wav image:", err);
        wavImageRef.current = null;
      });
  }, [ampRange, devicePixelRatio, height, idChStr, startSec, width, pxPerSec, drawWavOnNextFrame]);

  if (prevGetWavImageRef.current === getWavImage && needRefreshWavImg) {
    getWavImage();
  }
  prevGetWavImageRef.current = getWavImage;

  useEffect(() => getWavImage(), [getWavImage]);

  useEffect(() => {
    if (blend >= 1 || !spectrogram) {
      wavCtxRef.current?.transferFromImageBitmap(null);
      return;
    }
    drawWavOnNextFrame();
  }, [drawWavOnNextFrame, blend, spectrogram]);

  const setLoadingDisplay = useCallback(() => {
    if (!loadingElem.current) return;
    loadingElem.current.style.display = needRefreshWavImg ? "block" : "none";
  }, [needRefreshWavImg]);

  const setLoadingDisplayRef = useRef(setLoadingDisplay);
  useEffect(() => {
    setLoadingDisplayRef.current = setLoadingDisplay;
    // Request a setLoadingDisplay only when the draw function or its dependencies change
    setTimeout(() => {
      // Ensure setLoadingDisplayRef.current exists and call it
      if (setLoadingDisplayRef.current) setLoadingDisplayRef.current();
    }, 100);
  }, [setLoadingDisplay]);

  // Cleanup WebGL resources on unmount or when canvas element changes
  useEffect(() => {
    return () => {
      const resources = webglResourcesRef.current;
      if (resources?.gl) cleanupWebGLResources(resources);

      webglResourcesRef.current = null; // Clear the ref
    };
  }, []);

  const getTooltipLines = useEvent(async (e: React.MouseEvent) => {
    if (!wavCanvasElem.current) return ["sec", "Hz"];
    const x = e.clientX - wavCanvasElem.current.getBoundingClientRect().left;
    const y = Math.min(
      Math.max(e.clientY - wavCanvasElem.current.getBoundingClientRect().top, 0),
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
        key="spec"
        className={styles.ImgCanvas}
        ref={specCanvasElemCallback}
        style={{zIndex: 0}}
        width={Math.max(1, Math.floor(width * devicePixelRatio))}
        height={Math.max(1, Math.floor(height * devicePixelRatio))}
      />
      <canvas
        key="wav"
        className={styles.ImgCanvas}
        ref={wavCanvasElemCallback}
        style={{zIndex: 1}}
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
