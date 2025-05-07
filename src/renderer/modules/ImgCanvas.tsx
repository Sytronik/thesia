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

    wavCtxRef.current = wavCanvasElem.current.getContext("bitmaprenderer", {alpha: true});

    if (!wavCtxRef.current) {
      console.error("Failed to get bitmaprenderer context.");
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
    if (hidden) return;
    throttledGetSpectrogram(startSec, width, pxPerSec, idChStr, hzRange);
  }, [hidden, hzRange, idChStr, pxPerSec, startSec, width, throttledGetSpectrogram]);

  // getSpectrogram is called when needRefresh is true ...
  const prevGetSpectrogramRef = useRef<() => void>(getSpectrogramIfNotHidden);
  if (prevGetSpectrogramRef.current === getSpectrogramIfNotHidden && needRefresh)
    getSpectrogramIfNotHidden();
  prevGetSpectrogramRef.current = getSpectrogramIfNotHidden;

  // or when deps change
  useEffect(() => {
    if (blend <= 0) return;
    getSpectrogramIfNotHidden();
  }, [blend, getSpectrogramIfNotHidden]);

  // drawWavImage is not updated when deps change
  // the actual render (transferFromImageBitmap) is called once at a frame
  const transferRequestRef = useRef<number>(0);
  const drawWavImage = useEvent(async () => {
    if (!wavCanvasElem.current || !wavCtxRef.current || !wavImageRef.current) return;
    const ctx = wavCtxRef.current;
    const imdata = new Uint8ClampedArray(wavImageRef.current.buf);
    wavCanvasElem.current.style.opacity = blend < 0.5 ? "1" : `${Math.min(2 - 2 * blend, 1)}`;
    const img = new ImageData(imdata, wavImageRef.current.width, wavImageRef.current.height);
    const bitmap = await createImageBitmap(img);
    if (transferRequestRef.current !== 0) cancelAnimationFrame(transferRequestRef.current);
    transferRequestRef.current = requestAnimationFrame(() => ctx.transferFromImageBitmap(bitmap));
  });

  // getWavImage is throttled and it calls drawWavImage always,
  // but inside drawWavImage, it renders the image once at a frame
  const throttledGetWavImage = useMemo(
    () =>
      throttle(
        1000 / 120,
        async (_idChStr, _startSec, _pxPerSec, _width, _height, _ampRange, _devicePixelRatio) => {
          const wavImage = await BackendAPI.getWavImage(
            _idChStr,
            _startSec,
            _pxPerSec,
            _width,
            _height,
            _ampRange,
            _devicePixelRatio,
          );
          if (wavImage === null) return;
          wavImageRef.current = wavImage;
          drawWavImage();
        },
      ),
    [drawWavImage],
  );
  const getWavImageIfNotHidden = useCallback(() => {
    if (hidden) return;
    throttledGetWavImage(idChStr, startSec, pxPerSec, width, height, ampRange, devicePixelRatio);
  }, [
    hidden,
    idChStr,
    startSec,
    throttledGetWavImage,
    pxPerSec,
    width,
    height,
    ampRange,
    devicePixelRatio,
  ]);

  // getWavImage is called when needRefresh is true ...
  const prevGetWavImageRef = useRef<() => void>(getWavImageIfNotHidden);
  if (prevGetWavImageRef.current === getWavImageIfNotHidden && needRefresh) {
    getWavImageIfNotHidden();
  }
  prevGetWavImageRef.current = getWavImageIfNotHidden;

  // or when deps change
  useEffect(() => {
    if (blend >= 1) return;
    getWavImageIfNotHidden();
  }, [blend, getWavImageIfNotHidden]);

  // update wavImage when blend or hidden changes
  useEffect(() => {
    if (blend >= 1 || hidden) {
      wavCtxRef.current?.transferFromImageBitmap(null);
      return;
    }

    drawWavImage();
  }, [blend, drawWavImage, hidden]);

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
        width={Math.max(1, Math.floor(width * devicePixelRatio))}
        height={Math.max(1, Math.floor(height * devicePixelRatio))}
      />
    </div>
  );
});
ImgCanvas.displayName = "ImgCanvas";

export default React.memo(ImgCanvas);
