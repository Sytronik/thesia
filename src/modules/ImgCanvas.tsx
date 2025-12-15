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
import {DevicePixelRatioContext} from "src/contexts";

import styles from "./ImgCanvas.module.scss";
import BackendAPI, {FreqScale} from "../api";
import {postMessageToWorker, NUM_WORKERS} from "../lib/worker-pool";
import SpecCanvas from "./SpecCanvas";

const idChStrToWorkerIndex = (idChStr: string) => {
  const [id, ch] = idChStr.split("_");
  return (Number(id) + Number(ch) * 100) % NUM_WORKERS;
};

type ImgCanvasProps = {
  idChStr: string;
  width: number;
  height: number;
  startSec: number;
  pxPerSec: number;
  maxTrackSec: number;
  maxTrackHz: number;
  freqScale: FreqScale;
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
    maxTrackSec,
    maxTrackHz,
    freqScale,
    hzRange,
    ampRange,
    blend,
    isLoading,
    needRefresh,
    hidden,
  } = props;
  const workerIndex = idChStrToWorkerIndex(idChStr);

  const needHideWav = blend >= 1 || hidden;

  const devicePixelRatio = useContext(DevicePixelRatioContext);

  const wavCanvasElem = useRef<HTMLCanvasElement | null>(null);

  const loadingElem = useRef<HTMLDivElement>(null);
  const tooltipElem = useRef<HTMLSpanElement>(null);
  const [initTooltipInfo, setInitTooltipInfo] = useState<ImgTooltipInfo | null>(null);

  const getBoundingClientRect = useEvent(() => {
    return wavCanvasElem.current?.getBoundingClientRect() ?? new DOMRect();
  });

  const imperativeInstanceRef = useRef<ImgCanvasHandleElement>({getBoundingClientRect});
  useImperativeHandle(ref, () => imperativeInstanceRef.current, []);

  const wavCanvasElemCallback = useCallback((elem: HTMLCanvasElement | null) => {
    wavCanvasElem.current = elem;

    if (!wavCanvasElem.current) return;
    const offscreenCanvas = wavCanvasElem.current.transferControlToOffscreen();
    postMessageToWorker(workerIndex, {type: "init", data: {idChStr, canvas: offscreenCanvas}}, [
      offscreenCanvas,
    ]);
  }, []);

  useEffect(() => {
    postMessageToWorker(workerIndex, {type: "setDevicePixelRatio", data: {devicePixelRatio}});
  }, [devicePixelRatio, workerIndex]);

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
  }, [blend, workerIndex, idChStr, width, height, startSec, pxPerSec, ampRange]);

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
  }, [drawWavImage, width, height, needHideWav, workerIndex, idChStr]);

  useEffect(() => {
    if (!needRefresh) return; // Changing idChStr without needRefresh does not happen

    BackendAPI.getWav(idChStr).then((wavInfo) => {
      if (wavInfo !== null) {
        postMessageToWorker(workerIndex, {type: "setWav", data: {idChStr, wavInfo}}, [
          wavInfo.wavArr.buffer,
        ]);
        drawWavImageRef.current?.();
      }
    });
    return () => {
      postMessageToWorker(workerIndex, {type: "removeWav", data: {idChStr}});
    };
  }, [idChStr, needRefresh, workerIndex]);

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
      <SpecCanvas
        idChStr={idChStr}
        width={width}
        height={height}
        startSec={startSec}
        pxPerSec={pxPerSec}
        maxTrackHz={maxTrackHz}
        freqScale={freqScale}
        hzRange={hzRange}
        blend={blend}
        needRefresh={needRefresh}
        hidden={hidden}
        workerIndex={workerIndex}
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
      />
    </div>
  );
});
ImgCanvas.displayName = "ImgCanvas";

export default React.memo(ImgCanvas);
