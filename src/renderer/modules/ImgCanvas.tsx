import React, {
  forwardRef,
  useRef,
  useImperativeHandle,
  useState,
  useContext,
  useMemo,
  useEffect,
} from "react";
import useEvent from "react-use-event-hook";
import {throttle} from "throttle-debounce";
import {DevicePixelRatioContext} from "renderer/contexts";
import {resize} from "pica-gpu";
import styles from "./ImgCanvas.module.scss";
import BackendAPI from "../api";

type ImgCanvasProps = {
  width: number;
  height: number;
  maxTrackSec: number;
  canvasIsFit: boolean;
  bmpBuffer: Buffer | null;
};

type ImgTooltipInfo = {pos: number[]; lines: string[]};

const calcTooltipPos = (e: React.MouseEvent) => {
  return [e.clientX + 0, e.clientY + 15];
};

const ImgCanvas = forwardRef((props: ImgCanvasProps, ref) => {
  const {width, height, maxTrackSec, canvasIsFit, bmpBuffer} = props;
  const devicePixelRatio = useContext(DevicePixelRatioContext);
  const canvasWrapperElem = useRef<HTMLDivElement>(null);
  const canvasElem = useRef<HTMLCanvasElement>(null);
  const loadingElem = useRef<HTMLDivElement>(null);
  const startSecRef = useRef<number>(0);
  const pxPerSecRef = useRef<number>(1);
  const tooltipElem = useRef<HTMLSpanElement>(null);
  const [initTooltipInfo, setInitTooltipInfo] = useState<ImgTooltipInfo | null>(null);

  const showLoading = useEvent(() => {
    if (loadingElem.current) loadingElem.current.style.display = "block";
  });

  const updateLensParams = useEvent((params: OptionalLensParams) => {
    startSecRef.current = params.startSec ?? startSecRef.current;
    pxPerSecRef.current = params.pxPerSec ?? pxPerSecRef.current;
  });

  const getBoundingClientRect = useEvent(() => {
    return canvasElem.current?.getBoundingClientRect() ?? new DOMRect();
  });

  const imperativeInstanceRef = useRef<ImgCanvasHandleElement>({
    showLoading,
    updateLensParams,
    getBoundingClientRect,
  });
  useImperativeHandle(ref, () => imperativeInstanceRef.current, []);

  const getTooltipLines = useEvent(async (e: React.MouseEvent) => {
    if (!canvasElem.current) return ["sec", "Hz"];
    const x = e.clientX - canvasElem.current.getBoundingClientRect().left;
    const y = Math.min(
      Math.max(e.clientY - canvasElem.current.getBoundingClientRect().top, 0),
      height,
    );
    // TODO: need better formatting (from backend?)
    const time = Math.min(Math.max(startSecRef.current + x / pxPerSecRef.current, 0), maxTrackSec);
    const timeStr = time.toFixed(6).slice(0, -3);
    const hz = await BackendAPI.freqPosToHzOnCurrentRange(y, height);
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
  const [bitmap, setBitmap] = useState<ImageBitmap | null>(null);

  useEffect(() => {
    if (!bmpBuffer) return;
    if (loadingElem.current) loadingElem.current.style.display = "none";

    const bmpData = new Uint8Array(bmpBuffer);
    const bmpBlob = new Blob([bmpData.buffer], {type: "image/bmp"});
    createImageBitmap(bmpBlob, {imageOrientation: "flipY"})
      .then((bmp) => {
        setBitmap(bmp);
      })
      .catch(() => {});
  }, [bmpBuffer]);

  useEffect(() => {
    if (!canvasElem.current || !bitmap) return;
    if (loadingElem.current) loadingElem.current.style.display = "none";
    resize(bitmap, canvasElem.current, {
      // filter: "lanczos3",
      filter: "mks2013",
      targetWidth: width * devicePixelRatio,
      targetHeight: height * devicePixelRatio,
    });
  }, [width, height, bitmap, devicePixelRatio]);

  return (
    <div
      className={styles.imgCanvasWrapper}
      ref={canvasWrapperElem}
      /* this is needed for consistent layout
         because changing width of canvas elem can occur in different time (in draw function) */
      style={{width, height}}
    >
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
        className={styles.ImgCanvas}
        ref={canvasElem}
        style={{width: "100%", height: "100%"}}
        onMouseEnter={async (e) => {
          if (e.buttons !== 0) return;
          setInitTooltipInfo({pos: calcTooltipPos(e), lines: await getTooltipLines(e)});
        }}
        onMouseMove={onMouseMove}
        onMouseLeave={() => setInitTooltipInfo(null)}
        width={width * devicePixelRatio}
        height={height * devicePixelRatio}
      />
    </div>
  );
});
ImgCanvas.displayName = "ImgCanvas";

export default React.memo(ImgCanvas);
