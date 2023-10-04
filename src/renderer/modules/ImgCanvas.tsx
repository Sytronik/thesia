import React, {forwardRef, useRef, useImperativeHandle, useState, useContext} from "react";
import useEvent from "react-use-event-hook";
import {throttle} from "throttle-debounce";
import {DevicePixelRatioContext} from "renderer/contexts";
import styles from "./ImgCanvas.scss";
import NativeAPI from "../api";

type ImgCanvasProps = {
  width: number;
  height: number;
  maxTrackSec: number;
  canvasIsFit: boolean;
};

const ImgCanvas = forwardRef((props: ImgCanvasProps, ref) => {
  const {width, height, maxTrackSec, canvasIsFit} = props;
  const devicePixelRatio = useContext(DevicePixelRatioContext);
  const canvasElem = useRef<HTMLCanvasElement>(null);
  const startSecRef = useRef<number>(0);
  const pxPerSecRef = useRef<number>(1);
  const [showTooltip, setShowTooltip] = useState<boolean>(false);
  const [tooltipText, setTooltipText] = useState<string>(" sec\n Hz");
  const [tooltipPosition, setTooltipPosition] = useState<[number, number]>([0, 0]);

  const draw = useEvent(async (buf: Buffer) => {
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

  const setTooltipPositionByCursorPos = useEvent((e: React.MouseEvent) => {
    setTooltipPosition([e.clientX + 10, e.clientY + 15]);
  });

  const onMouseMove = throttle(1000 / 120, async (e: React.MouseEvent) => {
    if (!showTooltip || !canvasElem.current) return;
    const x = e.clientX - canvasElem.current.getBoundingClientRect().left;
    const y = Math.min(
      Math.max(e.clientY - canvasElem.current.getBoundingClientRect().top, 0),
      height,
    );
    const time = Math.min(Math.max(startSecRef.current + x / pxPerSecRef.current, 0), maxTrackSec);
    const hz = await NativeAPI.getHzAtPointer(y, height);
    setTooltipText(`${time.toFixed(3)} sec\n${hz.toFixed(0)} Hz`); // TODO: need better formatting (from backend?)
    setTooltipPositionByCursorPos(e);
  });

  return (
    <div
      // this is needed for consistent layout
      // because changing width of canvas elem can occur in different time (in draw function)
      style={{width, height}}
    >
      {showTooltip ? (
        <span
          className={styles.tooltip}
          style={{
            left: `${tooltipPosition[0]}px`,
            top: `${tooltipPosition[1]}px`,
          }}
        >
          {tooltipText.split("\n").map((v) => (
            <p key={`tooltip_line${v.split(" ")[1]}`}>{v}</p>
          ))}
        </span>
      ) : null}
      <canvas
        className={styles.ImgCanvas}
        ref={canvasElem}
        // code for setting width is in draw function.
        // different height between image and canvas can be allowed.
        // the same for width only if canvasIsFit
        style={canvasIsFit ? {width, height} : {height}}
        onMouseEnter={(e) => {
          if (e.buttons !== 0) return;
          setShowTooltip(true);
          setTooltipPositionByCursorPos(e);
        }}
        onMouseMove={onMouseMove}
        onMouseLeave={() => setShowTooltip(false)}
      />
    </div>
  );
});
ImgCanvas.displayName = "ImgCanvas";

export default React.memo(ImgCanvas);
