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
import {debounce, throttle} from "throttle-debounce";
import {DevicePixelRatioContext} from "renderer/contexts";

import {
  WAV_BORDER_COLOR,
  WAV_CLIPPING_COLOR,
  WAV_COLOR,
} from "renderer/prototypes/constants/colors";
import {
  WAV_BORDER_WIDTH,
  WAV_IMAGE_SCALE,
  WAV_LINE_WIDTH_FACTOR,
  WAV_MARGIN_RATIO,
} from "renderer/prototypes/constants/tracks";
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

const clipFn = (clipValues: [number, number] | null) => {
  if (!clipValues) return (v: number) => v;
  const [min, max] = clipValues;
  return (v: number) => Math.min(Math.max(v, min), max);
};

const setLinePath = (
  ctx: CanvasRenderingContext2D,
  points: Float32Array,
  startPx: number,
  pxPerPoints: number,
  scaleY: number,
  clipValues: [number, number] | null = null,
) => {
  const clip = clipFn(clipValues);
  ctx.moveTo(startPx, clip(points[0]) * scaleY);
  ctx.beginPath();
  points.forEach((v, i) => {
    if (i === 0) return;
    ctx.lineTo(startPx + i * pxPerPoints, clip(v) * scaleY);
  });
};

const drawWavLine = (
  ctx: CanvasRenderingContext2D,
  wavLine: Float32Array,
  startPx: number,
  pxPerPoints: number,
  height: number,
  scale: number,
  devicePixelRatio: number,
  color: string,
  clipValues: [number, number] | null = null,
) => {
  ctx.lineCap = "round";
  ctx.lineJoin = "round";

  // border
  ctx.strokeStyle = WAV_BORDER_COLOR;
  ctx.lineWidth = WAV_LINE_WIDTH_FACTOR * scale + 2 * WAV_BORDER_WIDTH * devicePixelRatio;
  setLinePath(ctx, wavLine, startPx, pxPerPoints, height * scale, clipValues);
  ctx.stroke();

  // line
  ctx.strokeStyle = color;
  ctx.lineWidth = WAV_LINE_WIDTH_FACTOR * scale;
  setLinePath(ctx, wavLine, startPx, pxPerPoints, height * scale, clipValues);
  ctx.stroke();
};

const setEnvelopePath = (
  ctx: CanvasRenderingContext2D,
  topEnvelope: Float32Array,
  bottomEnvelope: Float32Array,
  startPx: number,
  pxPerPoints: number,
  scaleY: number,
  clipValues: [number, number] | null = null,
) => {
  const clip = clipFn(clipValues);
  ctx.moveTo(startPx, clip(topEnvelope[0]));
  ctx.beginPath();
  for (let i = 1; i < topEnvelope.length; i += 1) {
    ctx.lineTo(startPx + i * pxPerPoints, clip(topEnvelope[i]) * scaleY);
  }
  for (let i = bottomEnvelope.length - 1; i >= 0; i -= 1) {
    ctx.lineTo(startPx + i * pxPerPoints, clip(bottomEnvelope[i]) * scaleY);
  }
  ctx.closePath();
};

const drawWavEnvelope = (
  ctx: CanvasRenderingContext2D,
  topEnvelope: Float32Array,
  bottomEnvelope: Float32Array,
  startPx: number,
  pxPerPoints: number,
  height: number,
  scale: number,
  devicePixelRatio: number,
  color: string,
  clipValues: [number, number] | null = null,
  needBorder: boolean = true,
) => {
  // fill
  ctx.fillStyle = color;
  setEnvelopePath(
    ctx,
    topEnvelope,
    bottomEnvelope,
    startPx,
    pxPerPoints,
    height * scale,
    clipValues,
  );
  ctx.fill();

  if (needBorder) {
    ctx.lineCap = "round";
    ctx.lineJoin = "round";
    ctx.strokeStyle = WAV_BORDER_COLOR;
    ctx.lineWidth = WAV_BORDER_WIDTH * devicePixelRatio;
    setEnvelopePath(
      ctx,
      topEnvelope,
      bottomEnvelope,
      startPx,
      pxPerPoints,
      height * scale,
      clipValues,
    );
    ctx.stroke();
  }
};

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

  const specIsNotNeeded = blend <= 0 || hidden;
  const needHideWav = blend >= 1 || hidden;
  const devicePixelRatio = useContext(DevicePixelRatioContext);
  const wavCanvasScale = devicePixelRatio * WAV_IMAGE_SCALE;

  const spectrogramRef = useRef<Spectrogram | null>(null);
  const wavDrawingInfoRef = useRef<WavDrawingInfo | null>(null);

  const specCanvasElem = useRef<HTMLCanvasElement | null>(null);
  const webglResourcesRef = useRef<WebGLResources | null>(null);
  const wavCanvasElem = useRef<HTMLCanvasElement | null>(null);
  const wavCtxRef = useRef<CanvasRenderingContext2D | null>(null);

  const loadingElem = useRef<HTMLDivElement>(null);
  const tooltipElem = useRef<HTMLSpanElement>(null);
  const [initTooltipInfo, setInitTooltipInfo] = useState<ImgTooltipInfo | null>(null);

  const getBoundingClientRect = useEvent(() => {
    return wavCanvasElem.current?.getBoundingClientRect() ?? new DOMRect();
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

    wavCtxRef.current = wavCanvasElem.current.getContext("2d", {alpha: true, desynchronized: true});

    if (!wavCtxRef.current) {
      console.error("Failed to get 2d context.");
      wavCtxRef.current = null;
    }
  }, []);

  const debouncedRenderSpecHighQuality = useMemo(
    () =>
      debounce(100, (spectrogram, srcLeft, srcTop, srcW, srcH, dstW, dstH, _blend) => {
        requestAnimationFrame(() => {
          if (!webglResourcesRef.current) return;
          renderSpectrogram(
            webglResourcesRef.current,
            spectrogram,
            srcLeft,
            srcTop,
            srcW,
            srcH,
            dstW,
            dstH,
            _blend,
            false,
          );
        });
      }),
    [],
  );

  const drawSpectrogram = useCallback(() => {
    // Ensure WebGL resources are ready
    if (!specCanvasElem.current || !webglResourcesRef.current) return;

    const spectrogram = spectrogramRef.current;

    // Check if img and img.data are valid before proceeding
    if (!spectrogram || hidden || startSec >= trackSec) {
      const {gl} = webglResourcesRef.current;
      gl.clearColor(0, 0, 0, 0); // Clear to transparent black
      gl.clear(gl.COLOR_BUFFER_BIT);
      return;
    }

    // widths
    const dstLengthSec = width / pxPerSec;
    const srcLeft = Math.max(
      spectrogram.leftMargin + (startSec - spectrogram.startSec) * spectrogram.pxPerSec,
      0,
    );
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
      true, // bilinear: low qality
    );
    debouncedRenderSpecHighQuality(spectrogram, srcLeft, srcTop, srcW, srcH, dstW, dstH, blend);
  }, [
    hidden,
    startSec,
    trackSec,
    width,
    pxPerSec,
    devicePixelRatio,
    height,
    blend,
    debouncedRenderSpecHighQuality,
  ]);

  // Draw spectrogram when props change
  // Use a ref to store the latest draw function
  const drawSpectrogramRef = useRef(drawSpectrogram);
  useEffect(() => {
    drawSpectrogramRef.current = drawSpectrogram;
    // Request a redraw only when the draw function or its dependencies change
    const requestId = requestAnimationFrame(() => drawSpectrogramRef.current?.());

    // Cleanup function to cancel the frame if the component unmounts
    // or if dependencies change again before the frame executes
    return () => cancelAnimationFrame(requestId);
  }, [drawSpectrogram]);

  // getSpectrogram is throttled and it calls drawSpectrogram once at a frame
  const drawNewSpectrogramRequestRef = useRef<number>(0);
  const throttledGetSpectrogram = useMemo(
    () =>
      throttle(1000 / 120, async (_startSec, _width, _pxPerSec, _idChStr, _hzRange) => {
        const endSec = _startSec + _width / _pxPerSec;
        const spectrogram = await BackendAPI.getSpectrogram(
          _idChStr,
          [_startSec, endSec],
          _hzRange,
          MARGIN_FOR_RESIZE,
        );
        spectrogramRef.current = spectrogram;
        if (drawNewSpectrogramRequestRef.current !== 0)
          cancelAnimationFrame(drawNewSpectrogramRequestRef.current);
        drawNewSpectrogramRequestRef.current = requestAnimationFrame(() =>
          drawSpectrogramRef.current?.(),
        );
      }),
    [],
  );
  const getSpectrogramIfNotHidden = useCallback(() => {
    if (specIsNotNeeded) return;
    throttledGetSpectrogram(startSec, width, pxPerSec, idChStr, hzRange);
  }, [specIsNotNeeded, throttledGetSpectrogram, startSec, width, pxPerSec, idChStr, hzRange]);

  // getSpectrogram is called when needRefresh is true ...
  const prevGetSpectrogramRef = useRef<() => void>(getSpectrogramIfNotHidden);
  if (prevGetSpectrogramRef.current === getSpectrogramIfNotHidden && needRefresh)
    getSpectrogramIfNotHidden();
  prevGetSpectrogramRef.current = getSpectrogramIfNotHidden;

  // or when deps change
  useEffect(getSpectrogramIfNotHidden, [getSpectrogramIfNotHidden]);

  // drawWavImage is not updated when deps change
  // the actual render (transferFromImageBitmap) is called once at a frame
  const drawWavImage = useCallback(() => {
    if (!wavCanvasElem.current || !wavCtxRef.current) return;

    wavCanvasElem.current.width = width * devicePixelRatio;
    wavCanvasElem.current.height = height * devicePixelRatio;

    // set opacity by blend
    wavCanvasElem.current.style.opacity = blend < 0.5 ? "1" : `${Math.max(2 - 2 * blend, 0)}`;

    const ctx = wavCtxRef.current;

    ctx.scale(devicePixelRatio / wavCanvasScale, devicePixelRatio / wavCanvasScale);

    if (!wavDrawingInfoRef.current) return;
    const wavDrawingInfo = wavDrawingInfoRef.current;

    // startSec > trackSec case
    if (wavDrawingInfo.line !== null && wavDrawingInfo.line.length === 0) {
      ctx.clearRect(0, 0, width * wavCanvasScale, height * wavCanvasScale);
      return;
    }

    // fillRect case
    if (!wavDrawingInfo.line && !wavDrawingInfo.topEnvelope && !wavDrawingInfo.bottomEnvelope) {
      ctx.fillStyle = WAV_COLOR;
      ctx.fillRect(0, 0, width * wavCanvasScale, height * wavCanvasScale);
      return;
    }

    const pxPerPoints = (pxPerSec * wavCanvasScale) / wavDrawingInfo.pointsPerSec;
    const startPx =
      -wavDrawingInfo.preMargin * pxPerPoints -
      (startSec - wavDrawingInfo.startSec) * pxPerSec * wavCanvasScale;
    // line case
    if (wavDrawingInfo.line) {
      ctx.clearRect(0, 0, width * wavCanvasScale, height * wavCanvasScale);

      if (wavDrawingInfo.clipValues) {
        drawWavLine(
          ctx,
          wavDrawingInfo.line,
          startPx,
          pxPerPoints,
          height,
          wavCanvasScale,
          devicePixelRatio,
          WAV_CLIPPING_COLOR,
        );
      }

      drawWavLine(
        ctx,
        wavDrawingInfo.line,
        startPx,
        pxPerPoints,
        height,
        wavCanvasScale,
        devicePixelRatio,
        WAV_COLOR,
        wavDrawingInfo.clipValues,
      );
    } else if (wavDrawingInfo.topEnvelope && wavDrawingInfo.bottomEnvelope) {
      // envelope case
      ctx.clearRect(0, 0, width * wavCanvasScale, height * wavCanvasScale);

      if (wavDrawingInfo.clipValues) {
        drawWavEnvelope(
          ctx,
          wavDrawingInfo.topEnvelope,
          wavDrawingInfo.bottomEnvelope,
          startPx,
          pxPerPoints,
          height,
          wavCanvasScale,
          devicePixelRatio,
          WAV_CLIPPING_COLOR,
        );
      }

      drawWavEnvelope(
        ctx,
        wavDrawingInfo.topEnvelope,
        wavDrawingInfo.bottomEnvelope,
        startPx,
        pxPerPoints,
        height,
        wavCanvasScale,
        devicePixelRatio,
        WAV_COLOR,
        wavDrawingInfo.clipValues,
        wavDrawingInfo.clipValues === null,
      );
    }
  }, [blend, devicePixelRatio, height, pxPerSec, startSec, wavCanvasScale, width]);

  // Draw spectrogram when props change
  // Use a ref to store the latest draw function
  const drawWavImageRef = useRef(drawWavImage);
  useEffect(() => {
    if (needHideWav) {
      if (wavCanvasElem.current) {
        wavCanvasElem.current.width = width * devicePixelRatio;
        wavCanvasElem.current.height = height * devicePixelRatio;
        wavCanvasElem.current.style.opacity = "0";
      }
      wavCtxRef.current?.clearRect(0, 0, width * wavCanvasScale, height * wavCanvasScale);
      return () => {};
    }
    drawWavImageRef.current = drawWavImage;
    // Request a redraw only when the draw function or its dependencies change
    const requestId = requestAnimationFrame(() => drawWavImageRef.current?.());

    // Cleanup function to cancel the frame if the component unmounts
    // or if dependencies change again before the frame executes
    return () => cancelAnimationFrame(requestId);
  }, [devicePixelRatio, drawWavImage, height, hidden, needHideWav, wavCanvasScale, width]);

  // getWavImage is throttled and it calls drawWavImage always,
  // but inside drawWavImage, it renders the image once at a frame
  const drawWavImageRequestRef = useRef<number>(0);
  const throttledGetWavImage = useMemo(
    () =>
      throttle(
        1000 / 120,
        async (_idChStr, _startSec, _pxPerSec, _width, _height, _ampRange, _devicePixelRatio) => {
          const endSec = _startSec + _width / _pxPerSec;
          const wavSlice = await BackendAPI.getWavDrawingInfo(
            _idChStr,
            [_startSec, endSec],
            _width,
            _height,
            _ampRange,
            _devicePixelRatio,
            WAV_MARGIN_RATIO,
          );
          if (wavSlice === null) return;
          wavDrawingInfoRef.current = wavSlice;
          if (drawWavImageRequestRef.current !== 0)
            cancelAnimationFrame(drawWavImageRequestRef.current);
          drawWavImageRequestRef.current = requestAnimationFrame(() => drawWavImageRef.current?.());
        },
      ),
    [],
  );
  const getWavImageIfNotHidden = useCallback(() => {
    if (needHideWav) return;
    throttledGetWavImage(idChStr, startSec, pxPerSec, width, height, ampRange, devicePixelRatio);
  }, [
    ampRange,
    devicePixelRatio,
    height,
    idChStr,
    needHideWav,
    pxPerSec,
    startSec,
    throttledGetWavImage,
    width,
  ]);

  // getWavImage is called when needRefresh is true ...
  const prevGetWavImageRef = useRef<() => void>(getWavImageIfNotHidden);
  if (prevGetWavImageRef.current === getWavImageIfNotHidden && needRefresh) {
    getWavImageIfNotHidden();
  }
  prevGetWavImageRef.current = getWavImageIfNotHidden;

  // or when deps change
  useEffect(getWavImageIfNotHidden, [getWavImageIfNotHidden]);

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
      {!hidden && (
        <canvas
          key="spec"
          className={styles.ImgCanvas}
          ref={specCanvasElemCallback}
          style={{zIndex: 0}}
          width={Math.max(1, Math.floor(width * devicePixelRatio))}
          height={Math.max(1, Math.floor(height * devicePixelRatio))}
        />
      )}
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
      />
    </div>
  );
});
ImgCanvas.displayName = "ImgCanvas";

export default React.memo(ImgCanvas);
