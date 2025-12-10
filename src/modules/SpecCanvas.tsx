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
import BackendAPI, { Mipmap } from "../api";
import {
  cleanupWebGLResources,
  WebGLResources,
  MARGIN_FOR_RESIZE,
  renderSpectrogram,
  prepareWebGLResources,
} from "../lib/webgl-helpers";
import { postMessageToWorker, onReturnMipmap, onSetSpectrogramDone } from "../lib/worker-pool";
import { DevicePixelRatioContext } from "src/contexts";

type SpecCanvasProps = {
  idChStr: string;
  width: number;
  height: number;
  startSec: number;
  pxPerSec: number;
  hzRange: [number, number];
  blend: number;
  needRefresh: boolean;
  hidden: boolean;
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
    hzRange,
    blend,
    needRefresh,
    hidden,
    specIsNotNeeded,
    workerIndex,
  } = props;

  const devicePixelRatio = useContext(DevicePixelRatioContext);

  const endSec = startSec + width / (pxPerSec + 1e-8);
  
  const [mipmapInfo, setMipmapInfo] = useState<MipmapInfo | null>(null);
  const nextMipmapInfoRef = useRef<MipmapInfo | null>(null);
  const mipmapRef = useRef<Mipmap | null>(null);

  const needClearSpec = hidden || width <= 0 || (mipmapInfo !== null && startSec >= mipmapInfo.trackSec);

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

  const getMipmapInfoAndRequestMipmap = useEvent(async (_startSec, _endSec, _hzRange, force: boolean = false) => {
    const _mipmapInfo = await BackendAPI.getMipmapInfo(idChStr, [_startSec, _endSec], _hzRange, MARGIN_FOR_RESIZE);
    // console.log("getMipmapInfoAndRequestMipmap", _mipmapInfo);
    if (!_mipmapInfo) return;
    if (
      force ||
      (nextMipmapInfoRef.current === null &&
        (_mipmapInfo.width !== mipmapInfo?.width ||
          _mipmapInfo.height !== mipmapInfo?.height)) ||
      (nextMipmapInfoRef.current !== null &&
        (_mipmapInfo.width !== nextMipmapInfoRef.current.width ||
          _mipmapInfo.height !== nextMipmapInfoRef.current.height))
    ) {
      nextMipmapInfoRef.current = _mipmapInfo;
      postMessageToWorker(workerIndex, {
        type: "getMipmap",
        data: {
          idChStr,
          width: _mipmapInfo.width,
          height: _mipmapInfo.height,
        },
      });
    } else {
      if (nextMipmapInfoRef.current !== null)
        nextMipmapInfoRef.current = _mipmapInfo;
      else
        setMipmapInfo(_mipmapInfo);
    }
  });

  const getMipmapInfoAndRequestMipmapForcely = useEvent(
    () => getMipmapInfoAndRequestMipmap(startSec, endSec, hzRange, true)
  );

  const setSpectrogram = useEvent((_idChStr: string, _workerIndex: number) => {
    BackendAPI.getSpectrogram(_idChStr).then((spectrogram) => {
      if (spectrogram !== null) {
        // console.log("setSpectrogram", _idChStr, _workerIndex);
        postMessageToWorker(
          _workerIndex,
          { type: "setSpectrogram", data: { idChStr: _idChStr, ...spectrogram } },
          [spectrogram.arr.buffer]
        );
        
      }
    });
    return () => {
    //   postMessageToWorker(workerIndex, {type: "removeSpectrogram", data: {idChStr}});
    };
  });

  useEffect(() => {
    return onSetSpectrogramDone(workerIndex, idChStr, getMipmapInfoAndRequestMipmapForcely);
  }, [workerIndex, idChStr, getMipmapInfoAndRequestMipmapForcely]);

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
    if (specIsNotNeeded) return;
    
    getMipmapInfoAndRequestMipmap(startSec, endSec, hzRange);
  }, [startSec, endSec, hzRange, getMipmapInfoAndRequestMipmap, specIsNotNeeded]);

  const renderSpecHighQuality = useEvent((slicedMipmap, srcLeft, srcTop, srcW, srcH, dstW, dstH, _blend) => {
    if (!webglResourcesRef.current || needClearSpec || specIsNotNeeded) return;
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

  const draw = useEvent(
    async (
      _mipmapInfo: MipmapInfo | null,
      _needClearSpec: boolean,
      _devicePixelRatio: number,
      _height: number,
      _blend: number
    ) => {
      if (!specCanvasElem.current) return;
      if (!webglResourcesRef.current)
        webglResourcesRef.current = prepareWebGLResources(
          specCanvasElem.current
        );

      // Ensure WebGL resources are ready
      if (!webglResourcesRef.current) return;

      // Check if mipmap exists before proceeding
      if (_needClearSpec) {
        const { gl } = webglResourcesRef.current;
        gl.clearColor(0, 0, 0, 0);
        gl.clear(gl.COLOR_BUFFER_BIT);
        return;
      }

      if (!_mipmapInfo || !mipmapRef.current) return;

      // console.log("draw", _mipmapInfo, startSec, width, pxPerSec);

      const mipmap = mipmapRef.current;
      const { sliceArgs, startSec: mipmapStartSec, trackSec } = _mipmapInfo;

      // slice the mipmap using the sliceArgs
      const slicedArr = new Float32Array(sliceArgs.width * sliceArgs.height);
      for (let y = 0; y < sliceArgs.height; y++) {
        slicedArr
          .subarray(y * sliceArgs.width, (y + 1) * sliceArgs.width)
          .set(
            mipmap.arr.subarray(
              (y + sliceArgs.top) * mipmap.width + sliceArgs.left,
              (y + sliceArgs.top) * mipmap.width +
                sliceArgs.left +
                sliceArgs.width
            )
          );
      }
      const slicedMipmap = {
        arr: slicedArr,
        width: sliceArgs.width,
        height: sliceArgs.height,
      };

      // widths
      const srcLeft = sliceArgs.leftMargin + (startSec - mipmapStartSec) * sliceArgs.pxPerSec;
      let srcW = width * (sliceArgs.pxPerSec / pxPerSec);
      let dstW = width * _devicePixelRatio;

      if (startSec + width / (pxPerSec + 1e-8) > trackSec)
        dstW = (trackSec - startSec) * pxPerSec * _devicePixelRatio;

      srcW = Math.max(0.5, srcW);
      dstW = Math.max(0.5, dstW);

      // heights
      const srcTop = sliceArgs.topMargin;
      const srcH = Math.max(
        0.5,
        sliceArgs.height - srcTop - sliceArgs.bottomMargin
      );
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
        true // bilinear: low qality
      );
      debouncedRenderSpecHighQuality(
        slicedMipmap,
        srcLeft,
        srcTop,
        srcW,
        srcH,
        dstW,
        dstH,
        blend
      );
    }
  );

  useLayoutEffect(() => {
    const requestId = requestAnimationFrame(
      () => draw(mipmapInfo, needClearSpec, devicePixelRatio, height, blend),
    );

    // Cleanup function to cancel the frame if the component unmounts
    // or if dependencies change again before the frame executes
    return () => cancelAnimationFrame(requestId);
  }, [draw, mipmapInfo, needClearSpec, devicePixelRatio, height, blend]);

  const updateMipmapAndRequestDraw = useEvent((mipmap: Mipmap | null) => {
    if (!mipmap) return;
    // console.log("updateMipmapAndRequestDraw", mipmap, nextMipmapInfoRef.current);
    if (
      nextMipmapInfoRef.current === null ||
      nextMipmapInfoRef.current.width !== mipmap.width ||
      nextMipmapInfoRef.current.height !== mipmap.height
    )
      return;
    mipmapRef.current = mipmap;
    setMipmapInfo(nextMipmapInfoRef.current);
    nextMipmapInfoRef.current = null;
  })

  useEffect(() => {
    return onReturnMipmap(workerIndex, idChStr, updateMipmapAndRequestDraw);
  }, [workerIndex, idChStr, updateMipmapAndRequestDraw]);

  // Cleanup WebGL resources on unmount
  useEffect(() => {
    return () => {
      const resources = webglResourcesRef.current;
      if (resources?.gl) cleanupWebGLResources(resources);

      webglResourcesRef.current = null; // Clear the ref
    };
  }, []);

  return (
    hidden ? null : (
      <canvas
        key="spec"
        className={styles.ImgCanvas}
        ref={specCanvasElemCallback}
        style={{zIndex: 0}}
        width={Math.max(1, Math.floor(width * devicePixelRatio))}
        height={Math.max(1, Math.floor(height * devicePixelRatio))}
      />
    )
  );
};

export default React.memo(SpecCanvas);
