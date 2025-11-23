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
import {DevicePixelRatioContext} from "src/contexts";

import {sleep} from "src/utils/time";
import styles from "./ImgCanvas.module.scss";
import BackendAPI, { Mipmap } from "../api";
import {
  cleanupWebGLResources,
  WebGLResources,
  MARGIN_FOR_RESIZE,
  renderSpectrogram,
  prepareWebGLResources,
} from "../lib/webgl-helpers";
import { postMessageToWorker, NUM_WORKERS, onReturnMipmap } from "../lib/worker-pool";

const idChStrToWorkerIndex = (idChStr: string) => {
  const [id, ch] = idChStr.split("_");
  return (Number(id) + Number(ch) * 100) % NUM_WORKERS;
}

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
  const workerIndex = idChStrToWorkerIndex(idChStr);

  const endSec = startSec + width / (pxPerSec + 1e-8);

  const needClearSpec = hidden || startSec >= trackSec || width <= 0;
  const specIsNotNeeded = blend <= 0 || hidden;
  const needHideWav = blend >= 1 || hidden;

  const devicePixelRatio = useContext(DevicePixelRatioContext);

  const mipmapInfoRef = useRef<MipmapInfo | null>(null);
  const mipmapRef = useRef<Mipmap | null>(null);

  const specCanvasElem = useRef<HTMLCanvasElement | null>(null);
  const webglResourcesRef = useRef<WebGLResources | null>(null);
  const wavCanvasElem = useRef<HTMLCanvasElement | null>(null);

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

    if (!wavCanvasElem.current) return;
    const offscreenCanvas = wavCanvasElem.current.transferControlToOffscreen();
    postMessageToWorker(
      workerIndex,
      {type: "init", data: {idChStr, canvas: offscreenCanvas}},
      [offscreenCanvas],
    );
  }, []);


  useEffect(() => {
    postMessageToWorker(workerIndex, {type: "setDevicePixelRatio", data: {devicePixelRatio}});
  }, [devicePixelRatio]);

  const renderSpecHighQuality = useEvent((slicedMipmap, srcLeft, srcTop, srcW, srcH, dstW, dstH, _blend) => {
    if (!webglResourcesRef.current || needClearSpec) return;
    renderSpectrogram(
      webglResourcesRef.current,
      slicedMipmap,
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
      debounce(100, (slicedMipmap, srcLeft, srcTop, srcW, srcH, dstW, dstH, _blend) =>
        requestAnimationFrame(() =>
          renderSpecHighQuality(slicedMipmap, srcLeft, srcTop, srcW, srcH, dstW, dstH, _blend),
        ),
      ),
    [renderSpecHighQuality],
  );

  const drawSpectrogram = useCallback(async () => {
    if (!specCanvasElem.current) return;
    if (!webglResourcesRef.current)
      webglResourcesRef.current = prepareWebGLResources(specCanvasElem.current);

    // Ensure WebGL resources are ready
    if (!webglResourcesRef.current) return;


    // Check if mipmap exists before proceeding
    if (!mipmapInfoRef.current || needClearSpec) {
      const {gl} = webglResourcesRef.current;
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      return;
    }

    // Wait for mipmap to be ready (TODO: draw using current mipmap or low-resolution mipmap if available)
    while (
      mipmapInfoRef.current &&
      (mipmapRef.current === null ||
        mipmapInfoRef.current.width !== mipmapRef.current.width ||
        mipmapInfoRef.current.height !== mipmapRef.current.height)
    ) {
      await sleep(1000 / 120);
    }

    // Check again if mipmap exists before proceeding
    if (!mipmapInfoRef.current || !mipmapRef.current || needClearSpec) {
      const {gl} = webglResourcesRef.current;
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      return;
    }

    const mipmap = mipmapRef.current;
    const {sliceArgs, startSec: mipmapStartSec} = mipmapInfoRef.current;

    // slice the mipmap using the sliceArgs
    const slicedArr = new Float32Array(sliceArgs.width * sliceArgs.height);
    for (let y = 0; y < sliceArgs.height; y++) {
      slicedArr.subarray(
        y * sliceArgs.width,
        (y + 1) * sliceArgs.width
      ).set(
        mipmap.arr.subarray(
          (y + sliceArgs.top) * mipmap.width + sliceArgs.left,
          (y + sliceArgs.top) * mipmap.width + sliceArgs.left + sliceArgs.width
        )
      );
    }
    const slicedMipmap = {arr: slicedArr, width: sliceArgs.width, height: sliceArgs.height};

    // widths
    const dstLengthSec = width / pxPerSec;
    const srcLeft = Math.max(
      sliceArgs.leftMargin + (startSec - mipmapStartSec) * sliceArgs.pxPerSec,
      0,
    );
    let srcW = dstLengthSec * sliceArgs.pxPerSec;
    let dstW = width * devicePixelRatio;
    if (srcLeft + srcW > sliceArgs.width) srcW = sliceArgs.width - srcLeft;

    if (startSec + dstLengthSec > trackSec)
      dstW = (trackSec - startSec) * pxPerSec * devicePixelRatio;

    srcW = Math.max(0.5, srcW);
    dstW = Math.max(0.5, dstW);

    // heights
    const srcTop = sliceArgs.topMargin;
    const srcH = Math.max(0.5, sliceArgs.height - srcTop - sliceArgs.bottomMargin);
    const dstH = Math.max(0.5, Math.floor(height * devicePixelRatio));

    if (srcW <= 0 || srcH <= 0 || dstW <= 0 || dstH <= 0) {
      console.error("Invalid dimensions for textures:", {srcW, srcH, dstW, dstH});
      return; // Skip rendering
    }
    renderSpectrogram(
      webglResourcesRef.current,
      slicedMipmap,
      srcLeft,
      srcTop,
      srcW,
      srcH,
      dstW,
      dstH,
      blend,
      true, // bilinear: low qality
    );
    debouncedRenderSpecHighQuality(slicedMipmap, srcLeft, srcTop, srcW, srcH, dstW, dstH, blend);
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

  const getSpectrogram = useEvent(async (_startSec, _endSec, _idChStr, _hzRange) => {
    const mipmapInfo = await BackendAPI.getMipmapInfo(_idChStr, [_startSec, _endSec], _hzRange, MARGIN_FOR_RESIZE);
    if (!mipmapInfo) return;
    mipmapInfoRef.current = mipmapInfo;
    if (mipmapInfo.width !== mipmapRef.current?.width || mipmapInfo.height !== mipmapRef.current?.height) {
    postMessageToWorker(workerIndex, {
      type: "getMipmap",
      data: {
        idChStr: _idChStr,
        width: mipmapInfo.width,
        height: mipmapInfo.height,
      },
    });
  }
  });

  useEffect(() => {
    const unsubscribe = onReturnMipmap(workerIndex, idChStr, (mipmap: Mipmap | null) => {
      if (!mipmap) return;
      mipmapRef.current = mipmap;
      if (drawNewSpectrogramRequestRef.current !== 0)
        cancelAnimationFrame(drawNewSpectrogramRequestRef.current);
      drawNewSpectrogramRequestRef.current = requestAnimationFrame(() =>
        drawSpectrogramRef.current?.(),
      );
    });
    return () => unsubscribe();
  }, [workerIndex, idChStr]);

  const throttledGetSpectrogram = useMemo(
    () =>
      throttle(1000 / 60, (_startSec, _endSec, _idChStr, _hzRange) => {
        getSpectrogram(_startSec, _endSec, _idChStr, _hzRange);
      }),
    [getSpectrogram],
  );
  const getSpectrogramIfNotHidden = useCallback(
    (force: boolean = false) => {
      // Even if specIsNotNeeded, need at least one spectrogram to draw black box with proper size
      if (specIsNotNeeded && (mipmapRef.current || !force)) return;
      throttledGetSpectrogram(startSec, endSec, idChStr, hzRange);
    },
    [specIsNotNeeded, throttledGetSpectrogram, startSec, endSec, idChStr, hzRange],
  );

  // getSpectrogram is called when needRefresh is true ...
  const prevGetSpectrogramRef = useRef<() => void>(() => {});
  if (prevGetSpectrogramRef.current === getSpectrogramIfNotHidden && needRefresh)
    getSpectrogramIfNotHidden(true);
  prevGetSpectrogramRef.current = getSpectrogramIfNotHidden;

  // or when deps change
  useEffect(getSpectrogramIfNotHidden, [getSpectrogramIfNotHidden]);

  const drawWavImage = useCallback(() => {
    if (!wavCanvasElem.current) return;

    // set opacity by blend
    wavCanvasElem.current.style.opacity = blend < 0.5 ? "1" : `${Math.max(2 - 2 * blend, 0)}`;

    postMessageToWorker(workerIndex, {
      type: "drawWav", 
      data: {
        idChStr,
        width,
        height,
        startSec,
        pxPerSec,
        ampRange,
      },
    });
  }, [width, height, blend, startSec, pxPerSec, ampRange, idChStr]);

  // Draw spectrogram when props change
  // Use a ref to store the latest draw function
  const drawWavImageRef = useRef(drawWavImage);

  const drawWavImageRequestRef = useRef<number>(0);
  if (!needHideWav && drawWavImageRef.current === drawWavImage) {
    if (drawWavImageRequestRef.current !== 0) cancelAnimationFrame(drawWavImageRequestRef.current);
    drawWavImageRequestRef.current = requestAnimationFrame(drawWavImage);
  }

  useEffect(() => {
    if (needHideWav) {
      if (wavCanvasElem.current) {
        wavCanvasElem.current.style.opacity = "0";
      }
      postMessageToWorker(workerIndex, {type: "clearWav", data: {idChStr, width, height}});
      return () => {};
    }
    drawWavImageRef.current = drawWavImage;
    // Request a redraw only when the draw function or its dependencies change
    const requestId = requestAnimationFrame(() => drawWavImageRef.current?.());

    // Cleanup function to cancel the frame if the component unmounts
    // or if dependencies change again before the frame executes
    return () => cancelAnimationFrame(requestId);
  }, [drawWavImage, width, height, needHideWav, devicePixelRatio]);

  useEffect(() => {
    if (!needRefresh) return; // Changing idChStr without needRefresh does not happen
    Promise.all([
      BackendAPI.getSpectrogram(idChStr),
      BackendAPI.getWav(idChStr),
    ]).then(([spectrogram, wavInfo]) => {
      if (spectrogram !== null) {
        postMessageToWorker(
          workerIndex,
          { type: "setSpectrogram", data: { idChStr, ...spectrogram } },
          [spectrogram.arr.buffer]
        );
        getSpectrogramIfNotHidden();
      }
      if (wavInfo !== null) {
        postMessageToWorker(
          workerIndex,
          { type: "setWav", data: { idChStr, wavInfo } },
          [wavInfo.wavArr.buffer]
        );
        drawWavImageRef.current?.();
      }
    });
    return () => {
      postMessageToWorker(workerIndex, {type: "removeWav", data: {idChStr}});
    };
  }, [idChStr, needRefresh]);


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
    const hz = await BackendAPI.freqPosToHz(y, height, hzRange);
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
