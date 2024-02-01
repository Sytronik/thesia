import React, {
  forwardRef,
  useRef,
  useImperativeHandle,
  useEffect,
  useCallback,
  useContext,
} from "react";
import useEvent from "react-use-event-hook";
import {DevicePixelRatioContext} from "renderer/contexts";
import {AXIS_STYLE, LABEL_HEIGHT_ADJUSTMENT} from "../prototypes/constants/tracks";
import styles from "./AxisCanvas.module.scss";

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
  className: "timeRuler" | "ampAxis" | "freqAxis" | "dBAxis";

  // after resized and before new markers are calculated, old markers should be shifted or zoomed?
  shiftWhenResize?: boolean;
};

const AxisCanvas = forwardRef((props: AxisCanvasProps, ref) => {
  const {width, height, axisPadding, markerPos, direction, className, shiftWhenResize} = props;
  const devicePixelRatio = useContext(DevicePixelRatioContext);
  const canvasElem = useRef<HTMLCanvasElement | null>(null);
  const ctxRef = useRef<CanvasRenderingContext2D | null>(null);
  const prevMarkersAndLengthRef = useRef<[Markers, number]>([[], 1]);
  const bgColor = useRef<string>("");

  const canvasElemCallback = useCallback((elem: HTMLCanvasElement | null) => {
    if (!elem) {
      canvasElem.current = null;
      return;
    }
    bgColor.current = window.getComputedStyle(elem).backgroundColor;
    canvasElem.current = elem;
  }, []);

  const correctMarkerPos = useEvent(
    (x: number, axisLength: number) =>
      Math.round(((x * (axisLength - LINE_WIDTH)) / axisLength) * devicePixelRatio) /
        devicePixelRatio +
      LINE_WIDTH / 2,
  );

  const draw = useEvent((markersAndLength: [Markers, number], forced = false) => {
    if (prevMarkersAndLengthRef.current === markersAndLength && !forced) return;

    const ctx = ctxRef.current;
    if (!ctx) return;

    ctx.save();
    ctx.fillStyle = bgColor.current;
    ctx.fillRect(0, 0, width, height);
    ctx.restore();

    const [markers, lenForMarkers] = markersAndLength;
    if (markers.length === 0) return;

    const {MAJOR_TICK_POS, MINOR_TICK_POS, LABEL_POS, LABEL_LEFT_MARGIN} = markerPos;

    const axisLength = (direction === "H" ? width : height) - 2 * axisPadding;
    const ratioToPx = shiftWhenResize ? lenForMarkers : axisLength;
    if (direction === "H") {
      ctx.beginPath();
      ctx.moveTo(axisPadding, height - LINE_WIDTH / 2);
      ctx.lineTo(width - axisPadding, height - LINE_WIDTH / 2);
      ctx.stroke();

      markers.forEach((marker) => {
        const [posRatio, label] = marker;
        const pxPosition = correctMarkerPos(posRatio * ratioToPx + axisPadding, axisLength);

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
      ctx.moveTo(LINE_WIDTH / 2, axisPadding);
      ctx.lineTo(LINE_WIDTH / 2, height - axisPadding);
      ctx.stroke();

      markers.forEach((marker) => {
        const [posRatio, label] = marker;
        const pxPosition = correctMarkerPos(posRatio * ratioToPx + axisPadding, axisLength);

        ctx.beginPath();
        if (label) {
          ctx.fillText(label, LABEL_POS + LABEL_LEFT_MARGIN, pxPosition - LABEL_HEIGHT_ADJUSTMENT);
          ctx.moveTo(MAJOR_TICK_POS, pxPosition);
        } else {
          ctx.moveTo(MINOR_TICK_POS, pxPosition);
        }
        ctx.lineTo(0, pxPosition);
        ctx.closePath();
        ctx.stroke();
      });
    }
    prevMarkersAndLengthRef.current = markersAndLength;
  });

  useEffect(() => {
    if (!canvasElem.current) return;

    canvasElem.current.width = width * devicePixelRatio;
    canvasElem.current.height = height * devicePixelRatio;

    const ctx = canvasElem.current.getContext("2d", {alpha: false, desynchronized: true});
    ctxRef.current = ctx;
    if (!ctx) return;
    ctx.scale(devicePixelRatio, devicePixelRatio);
    ctx.fillStyle = LABEL_COLOR;
    ctx.strokeStyle = TICK_COLOR;
    ctx.lineWidth = LINE_WIDTH;
    ctx.font = LABEL_FONT;
    ctx.textBaseline = "hanging";

    draw(prevMarkersAndLengthRef.current, true);
  }, [width, height, devicePixelRatio, draw]);

  const imperativeInstanceRef = useRef<AxisCanvasHandleElement>({draw});
  useImperativeHandle(ref, () => imperativeInstanceRef.current, []);

  return (
    <canvas
      className={`AxisCanvas ${styles[className]}`}
      ref={canvasElemCallback}
      style={{width, height}}
    />
  );
});
AxisCanvas.displayName = "AxisCanvas";
AxisCanvas.defaultProps = {
  shiftWhenResize: false,
};

export default React.memo(AxisCanvas);
