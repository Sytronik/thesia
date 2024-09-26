import React, {forwardRef, useRef, useImperativeHandle, useState, useContext} from "react";
import useEvent from "react-use-event-hook";
import {throttle} from "throttle-debounce";
import {DevicePixelRatioContext} from "renderer/contexts";
import styles from "./ImgCanvas.module.scss";
import BackendAPI from "../api";

type ImgCanvasProps = {
  width: number;
  height: number;
  maxTrackSec: number;
  canvasIsFit: boolean;
};

type ImgTooltipInfo = {pos: number[]; lines: string[]};

const calcTooltipPos = (e: React.MouseEvent) => {
  return [e.clientX + 0, e.clientY + 15];
};

const ImgCanvas = forwardRef((props: ImgCanvasProps, ref) => {
  const {width, height, maxTrackSec, canvasIsFit} = props;
  const devicePixelRatio = useContext(DevicePixelRatioContext);
  const canvasElem = useRef<HTMLCanvasElement>(null);
  const startSecRef = useRef<number>(0);
  const pxPerSecRef = useRef<number>(1);
  const tooltipElem = useRef<HTMLSpanElement>(null);
  const [initTooltipInfo, setInitTooltipInfo] = useState<ImgTooltipInfo | null>(null);

  const draw = useEvent((buf: Buffer | null) => {
    if (buf === null) {
      const ctx = canvasElem.current?.getContext("bitmaprenderer");
      ctx?.transferFromImageBitmap(null);
      // TODO: loading image
      return;
    }
    const bitmapWidth = width * devicePixelRatio;
    const bitmapHeight = height * devicePixelRatio;
    if (buf.byteLength !== 4 * bitmapWidth * bitmapHeight) return;

    const ctx = canvasElem.current?.getContext("bitmaprenderer");
    if (!ctx) return;

    const imdata = new ImageData(new Uint8ClampedArray(buf), bitmapWidth, bitmapHeight);
    createImageBitmap(imdata)
      .then((imbmp) => {
        // to make the size of canvas the same as that of imdata
        if (!canvasIsFit && canvasElem.current) {
          canvasElem.current.style.width = `${bitmapWidth / devicePixelRatio}px`;
          // height is set in JSX
        }
        ctx.transferFromImageBitmap(imbmp);
      })
      .catch(() => {});
  });

  const updateLensParams = useEvent((params: OptionalLensParams) => {
    startSecRef.current = params.startSec ?? startSecRef.current;
    pxPerSecRef.current = params.pxPerSec ?? pxPerSecRef.current;
  });

  const getBoundingClientRect = useEvent(() => {
    return canvasElem.current?.getBoundingClientRect() ?? new DOMRect();
  });

  const imperativeInstanceRef = useRef<ImgCanvasHandleElement>({
    draw,
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

  const onMouseMove = throttle(1000 / 120, async (e: React.MouseEvent) => {
    if (initTooltipInfo === null || tooltipElem.current === null) return;
    const [left, top] = calcTooltipPos(e);
    tooltipElem.current.style.left = `${left}px`;
    tooltipElem.current.style.top = `${top}px`;
    const lines = await getTooltipLines(e);
    lines.forEach((v, i) => {
      const node = tooltipElem.current?.children.item(i) ?? null;
      if (node) node.innerHTML = v;
    });
  });

  return (
    <div
      className={styles.imgCanvasWrapper}
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
      <canvas
        className={styles.ImgCanvas}
        ref={canvasElem}
        /* code for setting width is in draw function.
           different height between image and canvas can be allowed.
           the same for width only if canvasIsFit */
        style={canvasIsFit ? {width, height} : {width: canvasElem.current?.style.width, height}}
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
