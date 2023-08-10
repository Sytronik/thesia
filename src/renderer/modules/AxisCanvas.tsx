import React, {forwardRef, useRef, useImperativeHandle, useEffect} from "react";
import {AXIS_STYLE, LABEL_HEIGHT_ADJUSTMENT} from "../prototypes/constants";
import styles from "./AxisCanvas.scss";

const {LINE_WIDTH, TICK_COLOR, LABEL_COLOR, LABEL_FONT} = AXIS_STYLE;

type MarkerPosition = {
  MAJOR_TICK_POS: number;
  MINOR_TICK_POS: number;
  LABEL_POS: number;
  LABEL_LEFT_MARGIN: number;
};

type AxisCanvasProps = {
  width: number;
  height: number;
  axisPadding: number;
  markerPos: MarkerPosition;
  direction: "H" | "V"; // stands for horizontal and vertical
  className: "timeRuler" | "ampAxis" | "freqAxis" | "dbAxis";
};

const AxisCanvas = forwardRef((props: AxisCanvasProps, ref) => {
  const {width, height, axisPadding, markerPos, direction, className} = props;
  const axisCanvasElem = useRef<HTMLCanvasElement>(null);
  const axisCanvasCtxRef = useRef<CanvasRenderingContext2D>();
  const {MAJOR_TICK_POS, MINOR_TICK_POS, LABEL_POS, LABEL_LEFT_MARGIN} = markerPos;

  useEffect(() => {
    const ratio = window.devicePixelRatio || 1;

    if (!axisCanvasElem.current) {
      return;
    }

    axisCanvasElem.current.width = width * ratio;
    axisCanvasElem.current.height = height * ratio;

    axisCanvasCtxRef.current = axisCanvasElem.current.getContext("2d") as CanvasRenderingContext2D;
    axisCanvasCtxRef.current.scale(ratio, ratio);
  }, [width, height]);

  useImperativeHandle(ref, () => ({
    draw: (markers: Markers) => {
      const ctx = axisCanvasCtxRef.current;

      if (!ctx || !markers?.length) {
        return;
      }

      ctx.clearRect(0, 0, width, height); // [TEMP]

      ctx.fillStyle = LABEL_COLOR;
      ctx.strokeStyle = TICK_COLOR;
      ctx.lineWidth = LINE_WIDTH;
      ctx.font = LABEL_FONT;
      ctx.textBaseline = "hanging";

      if (direction === "H") {
        ctx.beginPath();
        ctx.moveTo(axisPadding, height);
        ctx.lineTo(width - axisPadding, height);
        ctx.stroke();

        markers.forEach((marker) => {
          const [axisPosition, label] = marker;
          const pxPosition = axisPosition + axisPadding;

          ctx.beginPath();
          if (label) {
            ctx.fillText(label, pxPosition + LABEL_LEFT_MARGIN, LABEL_POS);
            ctx.moveTo(pxPosition, MAJOR_TICK_POS);
          } else {
            ctx.moveTo(pxPosition, MINOR_TICK_POS);
          }
          ctx.lineTo(pxPosition, height);
          ctx.closePath();
          ctx.stroke();
        });
      } else {
        ctx.beginPath();
        ctx.moveTo(0, axisPadding);
        ctx.lineTo(0, height - axisPadding);
        ctx.stroke();

        markers.forEach((marker) => {
          const [axisPosition, label] = marker;
          const pxPosition = axisPosition + axisPadding;

          ctx.beginPath();
          if (label) {
            ctx.fillText(
              label,
              LABEL_POS + LABEL_LEFT_MARGIN,
              pxPosition - LABEL_HEIGHT_ADJUSTMENT,
            );
            ctx.moveTo(MAJOR_TICK_POS, pxPosition);
          } else {
            ctx.moveTo(MINOR_TICK_POS, pxPosition);
          }
          ctx.lineTo(0, pxPosition);
          ctx.closePath();
          ctx.stroke();
        });
      }
    },
  }));

  return (
    <>
      <canvas
        className={`AxisCanvas ${styles[className]}`}
        ref={axisCanvasElem}
        style={{width, height}}
      />
    </>
  );
});
AxisCanvas.displayName = "AxisCanvas";

export default AxisCanvas;
