import React, {forwardRef, useRef, useImperativeHandle, useEffect} from "react";
import PROPERTY from "../prototypes/constants";
import styles from "./AxisCanvas.scss";

const {LINE_WIDTH, TICK_COLOR, LABEL_COLOR, LABEL_FONT} = PROPERTY.AXIS_STYLE;

const AxisCanvas = forwardRef(({width, height, markerPos, direction, className}, ref) => {
  const axisCanvasElem = useRef();
  const axisCanvasCtxRef = useRef();
  const {MAJOR_TICK_POS, MINOR_TICK_POS, LABEL_POS, LABEL_LEFT_MARGIN} = markerPos;

  useEffect(() => {
    const ratio = window.devicePixelRatio || 1;

    axisCanvasElem.current.width = width * ratio;
    axisCanvasElem.current.height = height * ratio;

    axisCanvasCtxRef.current = axisCanvasElem.current.getContext("2d");
    axisCanvasCtxRef.current.scale(ratio, ratio);
  }, [width, height]);

  useImperativeHandle(ref, () => ({
    draw: (markers) => {
      const ctx = axisCanvasCtxRef.current;
      ctx.clearRect(0, 0, width, height); // [TEMP]
      if (!markers) return;

      ctx.fillStyle = LABEL_COLOR;
      ctx.strokeStyle = TICK_COLOR;
      ctx.lineWidth = LINE_WIDTH;
      ctx.font = LABEL_FONT;
      ctx.textBaseline = "hanging";

      markers.forEach((marker) => {
        const [pxPosition, label] = marker;
        let xPxPosition = [];
        let yPxPosition = [];
        if (direction === "H") {
          xPxPosition = [pxPosition, pxPosition, pxPosition, pxPosition];
          yPxPosition = [LABEL_POS, MAJOR_TICK_POS, MINOR_TICK_POS, height];
        } else {
          xPxPosition = [LABEL_POS, MAJOR_TICK_POS, MINOR_TICK_POS, 0];
          yPxPosition = [pxPosition, pxPosition, pxPosition, pxPosition];
        }

        ctx.beginPath();
        if (label) {
          ctx.fillText(label, xPxPosition[0] + LABEL_LEFT_MARGIN, yPxPosition[0]);
          ctx.moveTo(xPxPosition[1], yPxPosition[1]);
        } else {
          ctx.moveTo(xPxPosition[2], yPxPosition[2]);
        }
        ctx.lineTo(xPxPosition[3], yPxPosition[3]);
        ctx.closePath();
        ctx.stroke();
      });
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
