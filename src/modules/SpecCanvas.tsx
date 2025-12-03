import React, {
  useRef,
  useMemo,
  useEffect,
  useCallback,
  useLayoutEffect,
  useContext,
} from "react";
import useEvent from "react-use-event-hook";
import {debounce, throttle} from "throttle-debounce";

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
import { postMessageToWorker, onReturnMipmap } from "../lib/worker-pool";
import { DevicePixelRatioContext } from "src/contexts";

type SpecCanvasProps = {
  idChStr: string;
  width: number;
  height: number;
  startSec: number;
  pxPerSec: number;
  trackSec: number;
  hzRange: [number, number];
  blend: number;
  needRefresh: boolean;
  needClearSpec: boolean;
  specIsNotNeeded: boolean;
  workerIndex: number;
};

const SpecCanvas = (props: SpecCanvasProps) => {
  const {
    idChStr,
    width,
    height,
    startSec,
    pxPerSec,
    trackSec,
    hzRange,
    blend,
    needRefresh,
    needClearSpec,
    specIsNotNeeded,
    workerIndex,
  } = props;

  const devicePixelRatio = useContext(DevicePixelRatioContext);

  const endSec = startSec + width / (pxPerSec + 1e-8);

  const mipmapInfoRef = useRef<MipmapInfo | null>(null);
  const mipmapRef = useRef<Mipmap | null>(null);

  const specCanvasElem = useRef<HTMLCanvasElement | null>(null);
  const webglResourcesRef = useRef<WebGLResources | null>(null);

  const specCanvasElemCallback = useCallback((elem: HTMLCanvasElement | null) => {
    // Cleanup previous resources if the element changes
    if (webglResourcesRef.current?.gl && elem !== specCanvasElem.current) {
      cleanupWebGLResources(webglResourcesRef.current);
    }

    specCanvasElem.current = elem;
    webglResourcesRef.current = null;
  }, []);

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

  useEffect(() => {
    if (!needRefresh) return; // Changing idChStr without needRefresh does not happen
    
    BackendAPI.getSpectrogram(idChStr).then((spectrogram) => {
      if (spectrogram !== null) {
        postMessageToWorker(
          workerIndex,
          { type: "setSpectrogram", data: { idChStr, ...spectrogram } },
          [spectrogram.arr.buffer]
        );
      }
    });
    return () => {
    //   postMessageToWorker(workerIndex, {type: "removeSpectrogram", data: {idChStr}});
    };
  }, [idChStr, needRefresh, workerIndex]);

  // Cleanup WebGL resources on unmount
  useEffect(() => {
    return () => {
      const resources = webglResourcesRef.current;
      if (resources?.gl) cleanupWebGLResources(resources);

      webglResourcesRef.current = null; // Clear the ref
    };
  }, []);

  return (
    <canvas
      key="spec"
      className={styles.ImgCanvas}
      ref={specCanvasElemCallback}
      style={{zIndex: 0}}
      width={Math.max(1, Math.floor(width * devicePixelRatio))}
      height={Math.max(1, Math.floor(height * devicePixelRatio))}
    />
  );
};

export default React.memo(SpecCanvas);
