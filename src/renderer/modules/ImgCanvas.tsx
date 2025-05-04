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
import {
  cleanupWebGLResources,
  WebGLResources,
  MARGIN_FOR_RESIZE,
  renderSpectrogram,
  prepareWebGLResources,
} from "../lib/webgl-helpers";

type ImgCanvasProps = {
  idChStr: string;
  width: number;
  height: number;
  startSec: number;
  pxPerSec: number;
  trackSec: number;
  maxTrackSec: number;
  hzRange: [number, number];
  ampRange: [number, number];
  blend: number;
  isLoading: boolean;
  needRefresh: boolean;
  hidden: boolean;
};

type ImgTooltipInfo = {pos: number[]; lines: string[]};

const calcTooltipPos = (e: React.MouseEvent) => [e.clientX + 0, e.clientY + 15];

const ImgCanvas = forwardRef((props: ImgCanvasProps, ref) => {
  const {
    idChStr,
    width,
    height,
    startSec,
    pxPerSec,
    trackSec,
    maxTrackSec,
    hzRange,
    ampRange,
    blend,
    isLoading,
    needRefresh,
    hidden,
  } = props;
  const devicePixelRatio = useContext(DevicePixelRatioContext);
  const spectrogramRef = useRef<Spectrogram | null>(null);
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

    webglResourcesRef.current = prepareWebGLResources(specCanvasElem.current);
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
    // Ensure WebGL resources are ready
    if (!specCanvasElem.current || !webglResourcesRef.current) return;

    const spectrogram = spectrogramRef.current;

    // Check if img and img.data are valid before proceeding
    if (!spectrogram || hidden || startSec >= trackSec || hzRange[0] >= hzRange[1]) {
      const {gl} = webglResourcesRef.current;
      gl.clearColor(0, 0, 0, 0); // Clear to transparent black
      gl.clear(gl.COLOR_BUFFER_BIT);
      return;
    }

    // widths
    const dstLengthSec = width / pxPerSec;
    const srcLeft = spectrogram.leftMargin;
    let srcW = dstLengthSec * spectrogram.pxPerSec;
    let dstW = width * devicePixelRatio;
    if (srcLeft + srcW > spectrogram.width) srcW = spectrogram.width - srcLeft;

    if (startSec + dstLengthSec > trackSec)
      dstW = (trackSec - startSec) * pxPerSec * devicePixelRatio;

    srcW = Math.max(0.5, srcW);
    dstW = Math.max(0.5, dstW);

    // heights
    const srcTop = spectrogram.topMargin;
    const srcH = Math.max(0.5, spectrogram.height - srcTop - spectrogram.bottomMargin);
    const dstH = Math.max(0.5, Math.floor(height * devicePixelRatio));

    if (srcW <= 0 || srcH <= 0 || dstW <= 0 || dstH <= 0) {
      console.error("Invalid dimensions for textures:", {srcW, srcH, dstW, dstH});
      return; // Skip rendering
    }
    renderSpectrogram(
      webglResourcesRef.current,
      spectrogram,
      srcLeft,
      srcTop,
      srcW,
      srcH,
      dstW,
      dstH,
      blend,
    );
  }, [hidden, startSec, trackSec, hzRange, width, pxPerSec, devicePixelRatio, height, blend]);

  const drawWav = useEvent(() => {
    if (!wavCanvasElem.current || !wavCtxRef.current || !wavImageRef.current) return;
    const ctx = wavCtxRef.current;
    const imdata = new Uint8ClampedArray(wavImageRef.current.buf);
    wavCanvasElem.current.style.opacity = blend < 0.5 ? "1" : `${Math.min(2 - 2 * blend, 1)}`;
    const img = new ImageData(imdata, wavImageRef.current.width, wavImageRef.current.height);
    createImageBitmap(img)
      .then((bitmap) => ctx.transferFromImageBitmap(bitmap))
      .catch((err) => console.error("Failed to transfer image bitmap:", err));
  });

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

  const getSpectrogram = useCallback(() => {
    const endSec = startSec + width / pxPerSec;
    BackendAPI.getSpectrogram(idChStr, [startSec, endSec], hzRange, MARGIN_FOR_RESIZE)
      .then((spectrogram) => {
        spectrogramRef.current = spectrogram;
        requestAnimationFrame(() => drawSpectrogramRef.current());
      })
      .catch((err) => console.error("Failed to get spectrogram:", err));
  }, [startSec, width, pxPerSec, hzRange, idChStr]);
  const prevGetSpectrogramRef = useRef<() => void>(getSpectrogram);

  if (prevGetSpectrogramRef.current === getSpectrogram && needRefresh) getSpectrogram();
  prevGetSpectrogramRef.current = getSpectrogram;

  useEffect(getSpectrogram, [getSpectrogram]);

  // Draw wav
  const lastWavTimestampRef = useRef<number>(-1);
  const drawWavOnNextFrame = useEvent((force: boolean = false) => {
    const animationFrameId = requestAnimationFrame((timestamp) => {
      if (timestamp === lastWavTimestampRef.current && !force) return;
      lastWavTimestampRef.current = timestamp;
      drawWav();
    });

    return () => cancelAnimationFrame(animationFrameId);
  });

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
  const prevGetWavImageRef = useRef<() => void>(getWavImage);

  if (prevGetWavImageRef.current === getWavImage && needRefresh) getWavImage();
  prevGetWavImageRef.current = getWavImage;

  useEffect(getWavImage, [getWavImage]);

  useEffect(() => {
    if (blend >= 1 || hidden) {
      wavCtxRef.current?.transferFromImageBitmap(null);
      return;
    }
    drawWavOnNextFrame();
  }, [drawWavOnNextFrame, blend, hidden]);

  const setLoadingDisplay = useCallback(() => {
    if (!loadingElem.current) return;
    loadingElem.current.style.display = isLoading ? "block" : "none";
  }, [isLoading]);

  const setLoadingDisplayRef = useRef(setLoadingDisplay);
  useEffect(() => {
    setLoadingDisplayRef.current = setLoadingDisplay;
    // Request a setLoadingDisplay only when the draw function or its dependencies change
    setTimeout(() => {
      // Ensure setLoadingDisplayRef.current exists and call it
      if (setLoadingDisplayRef.current) setLoadingDisplayRef.current();
    }, 500);
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
