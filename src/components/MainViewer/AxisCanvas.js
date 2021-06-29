import React, {forwardRef, useRef, useImperativeHandle} from "react";
import "./AxisCanvas.scss";
import {PROPERTY} from "../Property";

const {LINE_WIDTH, TICK_COLOR, LABEL_COLOR, LABEL_FONT} = PROPERTY.AXIS_STYLE;

const AxisCanvas = forwardRef(({width, height, markerPos}, ref) => {
  const timeRulerCanvasElem = useRef();
  const {MAJOR_TICK_POS, MINOR_TICK_POS, LABEL_POS, LABEL_LEFT_MARGIN} = markerPos;

  useImperativeHandle(ref, () => ({
    draw: (markers) => {
      const ctx = timeRulerCanvasElem.current.getContext("2d");
      ctx.clearRect(0, 0, width, height); // [TEMP]

      ctx.fillStyle = LABEL_COLOR;
      ctx.strokeStyle = TICK_COLOR;
      ctx.lineWidth = LINE_WIDTH;
      ctx.font = LABEL_FONT;
      ctx.textBaseline = "hanging";

      for (const [pxPosition, label] of markers) {
        // 가로 세로에 따라서 swap
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
      }
    },
  }));

  return (
    <>
      <canvas className="time-ruler" ref={timeRulerCanvasElem} height={height} width={width} />
    </>
  );
});

export default AxisCanvas;
