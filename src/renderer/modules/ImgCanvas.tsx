import React, {
  forwardRef,
  useRef,
  useImperativeHandle,
  useState,
  useContext,
  useMemo,
  useEffect,
  useCallback,
  useLayoutEffect,
} from "react";
import useEvent from "react-use-event-hook";
import {debounce, throttle} from "throttle-debounce";
import {DevicePixelRatioContext} from "renderer/contexts";

import {WAV_CLIPPING_COLOR, WAV_COLOR} from "renderer/prototypes/constants/colors";
import {
  WAV_IMAGE_SCALE,
  WAV_LINE_WIDTH_FACTOR,
  WAV_MARGIN_RATIO,
} from "renderer/prototypes/constants/tracks";
import {drawWavEnvelope, drawWavLine} from "renderer/lib/drawing-wav";
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
  const endSec = startSec + width / (pxPerSec + 1e-8);

  const needClearSpec = hidden || startSec >= trackSec || width <= 0;
  const specIsNotNeeded = blend <= 0 || hidden;
  const needHideWav = blend >= 1 || hidden;

  const devicePixelRatio = useContext(DevicePixelRatioContext);
  const wavCanvasScale = devicePixelRatio * WAV_IMAGE_SCALE;
  const scaledWidth = width * wavCanvasScale;
  const scaledHeight = height * wavCanvasScale;

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
    }

    specCanvasElem.current = elem;
    webglResourcesRef.current = null;
  }, []);

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

  const renderSpecHighQuality = useEvent((srcLeft, srcTop, srcW, srcH, dstW, dstH, _blend) => {
    if (!webglResourcesRef.current || !spectrogramRef.current || needClearSpec) return;
    renderSpectrogram(
      webglResourcesRef.current,
      spectrogramRef.current,
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

  const debouncedRenderSpecHighQuality = useMemo(
    () =>
      debounce(100, (srcLeft, srcTop, srcW, srcH, dstW, dstH, _blend) =>
        requestAnimationFrame(() =>
          renderSpecHighQuality(srcLeft, srcTop, srcW, srcH, dstW, dstH, _blend),
        ),
      ),
    [renderSpecHighQuality],
  );

  const drawSpectrogram = useCallback(() => {
    if (!specCanvasElem.current) return;
    if (!webglResourcesRef.current)
      webglResourcesRef.current = prepareWebGLResources(specCanvasElem.current);

    // Ensure WebGL resources are ready
    if (!webglResourcesRef.current) return;

    const spectrogram = spectrogramRef.current;

    // Check if img and img.data are valid before proceeding
    if (!spectrogram || needClearSpec) {
      const {gl} = webglResourcesRef.current;
      gl.clearColor(0, 0, 0, 0);
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
    debouncedRenderSpecHighQuality(srcLeft, srcTop, srcW, srcH, dstW, dstH, blend);
  }, [
    needClearSpec,
    width,
    pxPerSec,
    startSec,
    devicePixelRatio,
    trackSec,
    height,
    blend,
    debouncedRenderSpecHighQuality,
  ]);

  // Draw spectrogram when props change
  // Use a ref to store the latest draw function
  const drawSpectrogramRef = useRef(drawSpectrogram);
  useLayoutEffect(() => {
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
      throttle(1000 / 60, async (_startSec, _endSec, _idChStr, _hzRange) => {
        const spectrogram = await BackendAPI.getSpectrogram(
          _idChStr,
          [_startSec, _endSec],
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
  const getSpectrogramIfNotHidden = useCallback(
    (force: boolean = false) => {
      // Even if specIsNotNeeded, need at least one spectrogram to draw black box with proper size
      if (specIsNotNeeded && (spectrogramRef.current || !force)) return;
      throttledGetSpectrogram(startSec, endSec, idChStr, hzRange);
    },
    [specIsNotNeeded, throttledGetSpectrogram, startSec, endSec, idChStr, hzRange],
  );

  // getSpectrogram is called when needRefresh is true ...
  const prevGetSpectrogramRef = useRef<() => void>(getSpectrogramIfNotHidden);
  if (prevGetSpectrogramRef.current === getSpectrogramIfNotHidden && needRefresh)
    getSpectrogramIfNotHidden(true);
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
      ctx.clearRect(0, 0, scaledWidth, scaledHeight);
      return;
    }

    // fillRect case
    if (!wavDrawingInfo.line && !wavDrawingInfo.topEnvelope && !wavDrawingInfo.bottomEnvelope) {
      ctx.fillStyle = WAV_COLOR;
      ctx.fillRect(0, 0, scaledWidth, scaledHeight);
      return;
    }

    const pxPerPoints = (pxPerSec * wavCanvasScale) / wavDrawingInfo.pointsPerSec;
    const startPx =
      -wavDrawingInfo.preMargin * pxPerPoints -
      (startSec - wavDrawingInfo.startSec) * pxPerSec * wavCanvasScale;
    const options = {
      startPx,
      pxPerPoints,
      height: scaledHeight,
      scale: wavCanvasScale,
      devicePixelRatio,
      needBorder: true,
    };
    ctx.clearRect(0, 0, scaledWidth, scaledHeight);
    if (wavDrawingInfo.line) {
      // line case

      if (wavDrawingInfo.clipValues) {
        drawWavLine(ctx, wavDrawingInfo.line, {...options, color: WAV_CLIPPING_COLOR});
      }

      drawWavLine(ctx, wavDrawingInfo.line, {
        ...options,
        color: WAV_COLOR,
        clipValues: wavDrawingInfo.clipValues,
      });
    } else if (wavDrawingInfo.topEnvelope && wavDrawingInfo.bottomEnvelope) {
      // envelope case

      if (wavDrawingInfo.clipValues) {
        drawWavEnvelope(ctx, wavDrawingInfo.topEnvelope, wavDrawingInfo.bottomEnvelope, {
          ...options,
          color: WAV_CLIPPING_COLOR,
        });
      }

      drawWavEnvelope(ctx, wavDrawingInfo.topEnvelope, wavDrawingInfo.bottomEnvelope, {
        ...options,
        color: WAV_COLOR,
        clipValues: wavDrawingInfo.clipValues,
        needBorder: wavDrawingInfo.clipValues === null,
      });
    }
  }, [
    blend,
    devicePixelRatio,
    height,
    pxPerSec,
    scaledHeight,
    scaledWidth,
    startSec,
    wavCanvasScale,
    width,
  ]);

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
      wavCtxRef.current?.clearRect(0, 0, scaledWidth, scaledHeight);
      return () => {};
    }
    drawWavImageRef.current = drawWavImage;
    // Request a redraw only when the draw function or its dependencies change
    const requestId = requestAnimationFrame(() => drawWavImageRef.current?.());

    // Cleanup function to cancel the frame if the component unmounts
    // or if dependencies change again before the frame executes
    return () => cancelAnimationFrame(requestId);
  }, [
    devicePixelRatio,
    drawWavImage,
    height,
    hidden,
    needHideWav,
    scaledHeight,
    scaledWidth,
    wavCanvasScale,
    width,
  ]);

  // getWavImage is throttled and it calls drawWavImage always,
  // but inside drawWavImage, it renders the image once at a frame
  const drawWavImageRequestRef = useRef<number>(0);
  const throttledGetWavDrawingInfo = useMemo(
    () =>
      throttle(
        1000 / 60,
        async (_idChStr, _startSec, _endSec, _width, _height, _ampRange, _devicePixelRatio) => {
          const wavSlice = await BackendAPI.getWavDrawingInfo(
            _idChStr,
            [_startSec, _endSec],
            _width,
            _height,
            _ampRange,
            WAV_LINE_WIDTH_FACTOR,
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
  const getWavDrawingInfoIfNotHidden = useCallback(() => {
    if (needHideWav) return;
    throttledGetWavDrawingInfo(
      idChStr,
      startSec,
      endSec,
      width,
      height,
      ampRange,
      devicePixelRatio,
    );
  }, [
    ampRange,
    devicePixelRatio,
    endSec,
    height,
    idChStr,
    needHideWav,
    startSec,
    throttledGetWavDrawingInfo,
    width,
  ]);

  // getWavDrawingInfo is called when needRefresh is true ...
  const prevGetWavDrawingInfoRef = useRef<() => void>(getWavDrawingInfoIfNotHidden);
  if (prevGetWavDrawingInfoRef.current === getWavDrawingInfoIfNotHidden && needRefresh) {
    getWavDrawingInfoIfNotHidden();
  }
  prevGetWavDrawingInfoRef.current = getWavDrawingInfoIfNotHidden;

  // or when deps change
  useEffect(getWavDrawingInfoIfNotHidden, [getWavDrawingInfoIfNotHidden]);

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

  // Cleanup WebGL resources on unmount
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
