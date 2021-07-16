import React, {forwardRef, useRef, useImperativeHandle} from "react";
import "./AxisCanvas.scss";
import {PROPERTY} from "../Property";

const {LINE_WIDTH, TICK_COLOR, LABEL_COLOR, LABEL_FONT} = PROPERTY.AXIS_STYLE;

const AxisCanvas = forwardRef(({width, height, markerPos, direction, className}, ref) => {
  const axisCanvasElem = useRef();
  const {MAJOR_TICK_POS, MINOR_TICK_POS, LABEL_POS, LABEL_LEFT_MARGIN} = markerPos;

  useImperativeHandle(ref, () => ({
    draw: (markers) => {
      const ctx = axisCanvasElem.current.getContext("2d");
      ctx.clearRect(0, 0, width, height); // [TEMP]
      if (!markers) return;

      ctx.fillStyle = LABEL_COLOR;
      ctx.strokeStyle = TICK_COLOR;
      ctx.lineWidth = LINE_WIDTH;
      ctx.font = LABEL_FONT;
      ctx.textBaseline = "hanging";

      for (const [pxPosition, label] of markers) {
        let xPxPosition = [];
        let yPxPosition = [];
        if (direction == "H") {
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
      }
    },
  }));

  return (
    <>
      <canvas
        className={`AxisCanvas ${className}`}
        ref={axisCanvasElem}
        height={height}
        width={width}
      />
    </>
  );
});

export default AxisCanvas;
