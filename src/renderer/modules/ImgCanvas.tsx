import React, {forwardRef, useRef, useImperativeHandle, useEffect, useState} from "react";
import useEvent from "react-use-event-hook";
import {throttle} from "throttle-debounce";
import styles from "./ImgCanvas.scss";
import NativeAPI from "../api";

type ImgCanvasProps = {
  width: number;
  height: number;
  maxTrackSec: number;
  pixelRatio: number;
};

const ImgCanvas = forwardRef((props: ImgCanvasProps, ref) => {
  const {width, height, maxTrackSec, pixelRatio} = props;
  const canvasElem = useRef<HTMLCanvasElement>(null);
  const startSecRef = useRef<number>(0);
  const pxPerSecRef = useRef<number>(1);
  const [showTooltip, setShowTooltip] = useState<boolean>(false);
  const [tooltipText, setTooltipText] = useState<string>(" sec\n Hz");
  const [tooltipPosition, setTooltipPosition] = useState<[number, number]>([0, 0]);

  useEffect(() => {
    if (!canvasElem.current) return;

    canvasElem.current.width = width * pixelRatio;
    canvasElem.current.height = height * pixelRatio;
  }, [width, height, pixelRatio]);

  const draw = useEvent(async (buf: Buffer) => {
    const bitmapWidth = width * pixelRatio;
    const bitmapHeight = height * pixelRatio;
    if (!(buf && buf.byteLength === 4 * bitmapWidth * bitmapHeight)) {
      return;
    }

    const ctx = canvasElem.current?.getContext("bitmaprenderer");
    if (!ctx) return;

    const imdata = new ImageData(new Uint8ClampedArray(buf), bitmapWidth, bitmapHeight);
    const imbmp = await createImageBitmap(imdata);
    ctx.transferFromImageBitmap(imbmp);
  });

  const updateLensParams = useEvent((params: OptionalLensParams) => {
    startSecRef.current = params.startSec ?? startSecRef.current;
    pxPerSecRef.current = params.pxPerSec ?? pxPerSecRef.current;
  });

  const imperativeInstanceRef = useRef<ImgCanvasHandleElement>({draw, updateLensParams});
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
    <div style={{width, height}}>
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
        style={{width, height}}
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
