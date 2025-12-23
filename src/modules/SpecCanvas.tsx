import React, {
  useRef,
  useMemo,
  useEffect,
  useCallback,
  useLayoutEffect,
  useContext,
  useState,
} from "react";
import useEvent from "react-use-event-hook";
import {debounce} from "throttle-debounce";

import styles from "./ImgCanvas.module.scss";
import BackendAPI, {FreqScale, Mipmap} from "../api";
import {
  cleanupWebGLResources,
  WebGLResources,
  MARGIN_FOR_RESIZE,
  renderSpectrogram,
  prepareWebGLResources,
  MAX_TEXTURE_SIZE,
} from "../lib/webgl-helpers";
import {postMessageToWorker, onReturnMipmap, onSetSpectrogramDone} from "../lib/worker-pool";
import {DevicePixelRatioContext} from "../contexts";
import {
  calcMipmapSize,
  createMipmapSizeArr,
  createSpectrogramSliceArgs,
} from "../lib/mipmap-helpers";

type SpecCanvasProps = {
  idChStr: string;
  width: number;
  height: number;
  startSec: number;
  pxPerSec: number;
  maxTrackHz: number;
  freqScale: FreqScale;
  hzRange: [number, number];
  blend: number;
  needRefresh: boolean;
  hidden: boolean;
  workerIndex: number;
};

const SpecCanvas = (props: SpecCanvasProps) => {
  const {
    idChStr,
    width,
    height,
    startSec,
    pxPerSec,
    maxTrackHz,
    freqScale,
    hzRange,
    blend,
    needRefresh,
    hidden,
    workerIndex,
  } = props;

  const devicePixelRatio = useContext(DevicePixelRatioContext);

  const trackSecRef = useRef<number>(0);
  const calcEndSec = useEvent(() =>
    Math.min(startSec + width / (pxPerSec + 1e-8), trackSecRef.current),
  );

  const mipmapSizeArrRef = useRef<[number, number][][]>([]);
  const [mipmapSize, setMipmapSize] = useState<[number, number] | null>(null);
  const nextMipmapSizeRef = useRef<[number, number] | null>(null);
  const mipmapRef = useRef<Mipmap | null>(null);

  const mipmapIsNotNeeded = hidden || blend <= 0 || width <= 0;
  const needClearSpec = width <= 0 || startSec >= trackSecRef.current;

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

  const calcMipmapSizeAndRequestMipmap = useEvent(
    (_startSec, _endSec, _hzRange, force: boolean = false) => {
      const _mipmapSize = calcMipmapSize(
        mipmapSizeArrRef.current,
        trackSecRef.current,
        [_startSec, _endSec],
        [0, maxTrackHz],
        _hzRange,
        MARGIN_FOR_RESIZE,
        freqScale,
        MAX_TEXTURE_SIZE,
      );
      // console.log("calcMipmapSizeAndRequestMipmap", MAX_TEXTURE_SIZE, mipmapSizeArrRef.current, _mipmapSize);
      if (!_mipmapSize) return;
      if (
        force ||
        (nextMipmapSizeRef.current === null &&
          (_mipmapSize[0] !== mipmapSize?.[0] || _mipmapSize[1] !== mipmapSize?.[1])) ||
        (nextMipmapSizeRef.current !== null &&
          (_mipmapSize[0] !== nextMipmapSizeRef.current[0] ||
            _mipmapSize[1] !== nextMipmapSizeRef.current[1]))
      ) {
        nextMipmapSizeRef.current = _mipmapSize;
        // console.log("request mipmap", _mipmapSize);
        postMessageToWorker(workerIndex, {
          type: "getMipmap",
          data: {
            idChStr,
            width: _mipmapSize[0],
            height: _mipmapSize[1],
          },
        });
      }
    },
  );

  const calcMipmapSizeAndRequestMipmapForcely = useEvent(() =>
    calcMipmapSizeAndRequestMipmap(startSec, calcEndSec(), hzRange, true),
  );

  const setSpectrogram = useEvent((_idChStr: string, _workerIndex: number) => {
    BackendAPI.getSpectrogram(_idChStr).then((spectrogram) => {
      if (spectrogram !== null) {
        trackSecRef.current = spectrogram.trackSec;
        mipmapSizeArrRef.current = createMipmapSizeArr(
          spectrogram.width,
          spectrogram.height,
          MAX_TEXTURE_SIZE,
        );
        // console.log("setSpectrogram", _idChStr, _workerIndex);
        postMessageToWorker(
          _workerIndex,
          {type: "setSpectrogram", data: {idChStr: _idChStr, ...spectrogram}},
          [spectrogram.arr.buffer],
        );
      }
    });
    return () => {
      postMessageToWorker(_workerIndex, {type: "removeSpectrogram", data: {idChStr: _idChStr}});
    };
  });

  useEffect(() => {
    return onSetSpectrogramDone(workerIndex, idChStr, calcMipmapSizeAndRequestMipmapForcely);
  }, [workerIndex, idChStr, calcMipmapSizeAndRequestMipmapForcely]);

  const setSpectrogramReactively = useCallback(
    () => setSpectrogram(idChStr, workerIndex),
    [idChStr, workerIndex, setSpectrogram],
  );

  const setSpectrogramReactivelyRef = useRef<() => void>(null);
  useEffect(() => {
    setSpectrogramReactivelyRef.current = setSpectrogramReactively;
    // console.log("setSpectrogramReactively");
    return setSpectrogramReactivelyRef.current?.();
  }, [setSpectrogramReactively]);

  if (needRefresh && setSpectrogramReactivelyRef.current === setSpectrogramReactively) {
    // console.log("setSpectrogram by needRefresh");
    setSpectrogram(idChStr, workerIndex);
  }

  useEffect(() => {
    if (mipmapIsNotNeeded) return;

    calcMipmapSizeAndRequestMipmap(startSec, calcEndSec(), hzRange);
  }, [startSec, calcEndSec, hzRange, calcMipmapSizeAndRequestMipmap, mipmapIsNotNeeded]);

  const renderSpecHighQuality = useEvent(
    (slicedMipmap, srcLeft, srcTop, srcW, srcH, dstW, dstH, _blend) => {
      if (!webglResourcesRef.current || needClearSpec || mipmapIsNotNeeded) return;
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
    },
  );

  const debouncedRenderSpecHighQuality = useMemo(
    () =>
      debounce(100, (slicedMipmap, srcLeft, srcTop, srcW, srcH, dstW, dstH, _blend) =>
        requestAnimationFrame(() =>
          renderSpecHighQuality(slicedMipmap, srcLeft, srcTop, srcW, srcH, dstW, dstH, _blend),
        ),
      ),
    [renderSpecHighQuality],
  );

  const draw = useEvent(
    (
      _startSec: number,
      _endSec: number,
      _hzRange: [number, number],
      _pxPerSec: number,
      _width: number,
      _height: number,
      _needClearSpec: boolean,
      _devicePixelRatio: number,
      _blend: number,
      _mipmapSize: [number, number] | null,
    ) => {
      if (_needClearSpec) {
        if (webglResourcesRef.current !== null) {
          const {gl} = webglResourcesRef.current;
          gl.clearColor(0, 0, 0, 0);
          gl.clear(gl.COLOR_BUFFER_BIT);
        }
        return;
      }

      if (!specCanvasElem.current) return;
      if (!webglResourcesRef.current)
        webglResourcesRef.current = prepareWebGLResources(specCanvasElem.current);

      // Ensure WebGL resources are ready
      if (!webglResourcesRef.current) return;

      if (!_mipmapSize || !mipmapRef.current) return;

      // console.log("draw", _mipmapSize, startSec, width, pxPerSec);

      const mipmap = mipmapRef.current;
      const sliceArgs = createSpectrogramSliceArgs(
        mipmap.width,
        mipmap.height,
        trackSecRef.current,
        [startSec, calcEndSec()],
        [0, maxTrackHz],
        hzRange,
        MARGIN_FOR_RESIZE,
        freqScale,
      );
      sliceArgs.width = Math.min(sliceArgs.width, MAX_TEXTURE_SIZE);

      // slice the mipmap using the sliceArgs
      const slicedArr = new Float32Array(sliceArgs.width * sliceArgs.height);
      for (let y = 0; y < sliceArgs.height; y++) {
        slicedArr
          .subarray(y * sliceArgs.width, (y + 1) * sliceArgs.width)
          .set(
            mipmap.arr.subarray(
              (y + sliceArgs.top) * mipmap.width + sliceArgs.left,
              (y + sliceArgs.top) * mipmap.width + sliceArgs.left + sliceArgs.width,
            ),
          );
      }
      const slicedMipmap: Mipmap = {
        arr: slicedArr,
        width: sliceArgs.width,
        height: sliceArgs.height,
      };

      // widths
      const srcLeft = sliceArgs.leftMargin;
      let srcW = width * (sliceArgs.pxPerSec / pxPerSec);
      let dstW = width * _devicePixelRatio;

      if (startSec + width / (pxPerSec + 1e-8) > trackSecRef.current) {
        srcW = (trackSecRef.current - startSec) * sliceArgs.pxPerSec;
        dstW = (trackSecRef.current - startSec) * pxPerSec * _devicePixelRatio;
      }

      srcW = Math.max(0.5, srcW);
      dstW = Math.max(0.5, dstW);

      // heights
      const srcTop = sliceArgs.topMargin;
      const srcH = Math.max(0.5, sliceArgs.height - srcTop - sliceArgs.bottomMargin);
      const dstH = Math.max(0.5, Math.floor(_height * _devicePixelRatio));

      if (srcW <= 0 || srcH <= 0 || dstW <= 0 || dstH <= 0) {
        console.error("Invalid dimensions for textures:", {
          srcW,
          srcH,
          dstW,
          dstH,
        });
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
    },
  );

  useLayoutEffect(() => {
    if (hidden) return () => {};
    const requestId = requestAnimationFrame(() =>
      draw(
        startSec,
        calcEndSec(),
        hzRange,
        pxPerSec,
        width,
        height,
        needClearSpec,
        devicePixelRatio,
        blend,
        mipmapSize,
      ),
    );

    // Cleanup function to cancel the frame if the component unmounts
    // or if dependencies change again before the frame executes
    return () => cancelAnimationFrame(requestId);
  }, [
    draw,
    startSec,
    calcEndSec,
    hzRange,
    pxPerSec,
    width,
    height,
    needClearSpec,
    devicePixelRatio,
    blend,
    mipmapSize,
    hidden,
  ]);

  const updateMipmap = useEvent((mipmap: Mipmap | null) => {
    if (!mipmap) return;
    // console.log("updateMipmap", mipmap, nextMipmapSizeRef.current);
    if (
      nextMipmapSizeRef.current === null ||
      nextMipmapSizeRef.current[0] !== mipmap.width ||
      nextMipmapSizeRef.current[1] !== mipmap.height
    )
      return;
    mipmapRef.current = mipmap;
    setMipmapSize(nextMipmapSizeRef.current);
    nextMipmapSizeRef.current = null;
  });

  useEffect(() => {
    return onReturnMipmap(workerIndex, idChStr, updateMipmap);
  }, [workerIndex, idChStr, updateMipmap]);

  // Cleanup WebGL resources on unmount
  useEffect(() => {
    return () => {
      const resources = webglResourcesRef.current;
      if (resources?.gl) cleanupWebGLResources(resources);

      webglResourcesRef.current = null; // Clear the ref
    };
  }, []);

  return hidden ? null : (
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
