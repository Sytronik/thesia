import React, {
  forwardRef,
  useRef,
  useImperativeHandle,
  useEffect,
  useCallback,
  useContext,
  useLayoutEffect,
  useState,
} from "react";
import { createPortal } from "react-dom";
import useEvent from "react-use-event-hook";
import { DevicePixelRatioContext } from "../contexts";
import { AXIS_STYLE } from "../prototypes/constants/tracks";
import styles from "./AxisCanvas.module.scss";
import BackendAPI, { AxisKind } from "../api";

const { LINE_WIDTH, TICK_COLOR, LABEL_COLOR, LABEL_FONT } = AXIS_STYLE;

const TOOLTIP_DELAY_MS = 500;
const TOOLTIP_HIDE_DELAY_MS = 500;
const TOOLTIP_X_OFFSET_FOR_VERTICAL = 29;
const TOOLTIP_Y_OFFSET_FOR_HORIZONTAL = 4;
const TOOLTIP_VIEWPORT_MARGIN = 4;

type TooltipAnchor = {
  clientX: number;
  clientY: number;
  axisPosition: number;
  axisLength: number;
};

type TooltipInfo = TooltipAnchor & {
  text: string;
  left: number;
  top: number;
};

type AxisCanvasProps = {
  id: number;
  width: number;
  height: number;
  axisPadding: number;
  markerPos: MarkerPosition;
  markersAndLength: [Markers, number];
  direction: "H" | "V"; // stands for horizontal and vertical
  className: AxisKind;
  endInclusive?: boolean;

  // after resized and before new markers are calculated, old markers should be shifted or zoomed?
  shiftWhenResize?: boolean;

  onWheel?: (e: WheelEvent) => void;
  onClick?: (e: React.MouseEvent) => void;
  formatTooltip?: (axisPosition: number, axisLength: number) => string;
};

const AxisCanvas = forwardRef(
  ({ endInclusive = false, shiftWhenResize = false, ...props }: AxisCanvasProps, ref) => {
    const {
      id,
      width,
      height,
      axisPadding,
      markerPos,
      markersAndLength,
      direction,
      className,
      onWheel,
      onClick,
      formatTooltip,
    } = props;
    const devicePixelRatio = useContext(DevicePixelRatioContext);
    const canvasElem = useRef<HTMLCanvasElement | null>(null);
    const bgColor = useRef<string>("");
    const tooltipTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const tooltipHideTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const tooltipAnchorRef = useRef<TooltipAnchor | null>(null);
    const tooltipVisibleRef = useRef(false);
    const tooltipSizeRef = useRef({ width: 0, height: 0 });
    const tooltipElemRef = useRef<HTMLSpanElement>(null);
    const [tooltip, setTooltip] = useState<TooltipInfo | null>(null);

    const clearTooltipTimer = useEvent(() => {
      if (tooltipTimerRef.current === null) return;
      clearTimeout(tooltipTimerRef.current);
      tooltipTimerRef.current = null;
    });

    const clearTooltipHideTimer = useEvent(() => {
      if (tooltipHideTimerRef.current === null) return;
      clearTimeout(tooltipHideTimerRef.current);
      tooltipHideTimerRef.current = null;
    });

    const calculateTooltipPosition = useEvent((clientX: number, clientY: number) => {
      const { width: tooltipWidth, height: tooltipHeight } = tooltipSizeRef.current;
      let left: number;
      let top: number;
      if (direction === "H") {
        left = clientX - tooltipWidth / 2;
        const preferredTop = clientY + TOOLTIP_Y_OFFSET_FOR_HORIZONTAL;
        top =
          preferredTop + tooltipHeight + TOOLTIP_VIEWPORT_MARGIN <= window.innerHeight
            ? preferredTop
            : clientY - tooltipHeight - TOOLTIP_Y_OFFSET_FOR_HORIZONTAL;
      } else {
        const preferredLeft = clientX + TOOLTIP_X_OFFSET_FOR_VERTICAL;
        left =
          preferredLeft + tooltipWidth + TOOLTIP_VIEWPORT_MARGIN <= window.innerWidth
            ? preferredLeft
            : clientX - tooltipWidth / 2 - TOOLTIP_X_OFFSET_FOR_VERTICAL;
        top = clientY - tooltipHeight / 2;
      }
      return {
        left: Math.max(
          TOOLTIP_VIEWPORT_MARGIN,
          Math.min(left, window.innerWidth - tooltipWidth - TOOLTIP_VIEWPORT_MARGIN),
        ),
        top: Math.max(
          TOOLTIP_VIEWPORT_MARGIN,
          Math.min(top, window.innerHeight - tooltipHeight - TOOLTIP_VIEWPORT_MARGIN),
        ),
      };
    });

    const showTooltip = useEvent((anchor: TooltipAnchor) => {
      if (!formatTooltip) return;
      const { left, top } = calculateTooltipPosition(anchor.clientX, anchor.clientY);
      tooltipVisibleRef.current = true;
      setTooltip({
        ...anchor,
        text: formatTooltip(anchor.axisPosition, anchor.axisLength),
        left,
        top,
      });
    });

    const getTooltipAnchor = useEvent((clientX: number, clientY: number): TooltipAnchor | null => {
      if (!canvasElem.current || !formatTooltip) return null;
      const rect = canvasElem.current.getBoundingClientRect();
      const rawPosition = direction === "H" ? clientX - rect.left : clientY - rect.top;
      const canvasLength = direction === "H" ? rect.width : rect.height;
      const axisLength = Math.max(canvasLength - 2 * axisPadding, 0);
      if (axisLength <= 0) return null;
      return {
        clientX: direction === "H" ? clientX : rect.left,
        clientY: direction === "H" ? rect.bottom : clientY,
        axisPosition: Math.min(Math.max(rawPosition - axisPadding, 0), axisLength),
        axisLength,
      };
    });

    const onTooltipMouseMove = useEvent((e: React.MouseEvent) => {
      clearTooltipHideTimer();
      const anchor = getTooltipAnchor(e.clientX, e.clientY);
      if (!anchor) return;
      tooltipAnchorRef.current = anchor;
      if (tooltipVisibleRef.current) {
        showTooltip(anchor);
        return;
      }
      if (tooltipTimerRef.current !== null) return;
      tooltipTimerRef.current = setTimeout(() => {
        tooltipTimerRef.current = null;
        if (tooltipAnchorRef.current) showTooltip(tooltipAnchorRef.current);
      }, TOOLTIP_DELAY_MS);
    });

    const onTooltipMouseLeave = useEvent(() => {
      clearTooltipTimer();
      tooltipAnchorRef.current = null;
      if (!tooltipVisibleRef.current) return;
      clearTooltipHideTimer();
      tooltipHideTimerRef.current = setTimeout(() => {
        tooltipHideTimerRef.current = null;
        tooltipVisibleRef.current = false;
        setTooltip(null);
      }, TOOLTIP_HIDE_DELAY_MS);
    });

    useEffect(() => {
      if (!tooltipVisibleRef.current || !tooltipAnchorRef.current) return;
      const { clientX, clientY } = tooltipAnchorRef.current;
      const anchor = getTooltipAnchor(clientX, clientY);
      if (!anchor) return;
      tooltipAnchorRef.current = anchor;
      showTooltip(anchor);
    }, [
      axisPadding,
      formatTooltip,
      getTooltipAnchor,
      height,
      markersAndLength,
      showTooltip,
      width,
    ]);

    useLayoutEffect(() => {
      if (!tooltip || !tooltipElemRef.current) return;
      const rect = tooltipElemRef.current.getBoundingClientRect();
      if (
        rect.width === tooltipSizeRef.current.width &&
        rect.height === tooltipSizeRef.current.height
      )
        return;
      tooltipSizeRef.current = { width: rect.width, height: rect.height };
      const { left, top } = calculateTooltipPosition(tooltip.clientX, tooltip.clientY);
      setTooltip((current) => (current ? { ...current, left, top } : null));
    }, [calculateTooltipPosition, tooltip]);

    useEffect(() => {
      return () => {
        clearTooltipTimer();
        clearTooltipHideTimer();
      };
    }, [clearTooltipHideTimer, clearTooltipTimer]);

    const canvasElemCallback = useCallback(
      (elem: HTMLCanvasElement | null) => {
        if (!elem) {
          if (onWheel) canvasElem.current?.removeEventListener("wheel", onWheel);
          canvasElem.current = null;
          return;
        }
        bgColor.current = window.getComputedStyle(elem).backgroundColor;
        if (onWheel) elem.addEventListener("wheel", onWheel, { passive: false });
        canvasElem.current = elem;
      },
      [onWheel],
    );

    const correctMarkerPos = useEvent((x: number, axisLength: number) => {
      const endCorrected = endInclusive ? (x * (axisLength - LINE_WIDTH)) / axisLength : x;
      return Math.round(endCorrected * devicePixelRatio) / devicePixelRatio + LINE_WIDTH / 2;
    });

    const draw = useCallback(() => {
      if (!canvasElem.current) return;
      canvasElem.current.width = width * devicePixelRatio;
      canvasElem.current.height = height * devicePixelRatio;
      const ctx = canvasElem.current.getContext("2d", { alpha: false, desynchronized: true });
      if (!ctx) return;

      ctx.scale(devicePixelRatio, devicePixelRatio);
      ctx.fillStyle = LABEL_COLOR;
      ctx.strokeStyle = TICK_COLOR;
      ctx.lineWidth = LINE_WIDTH;
      ctx.font = LABEL_FONT;
      ctx.textBaseline = direction === "H" ? "alphabetic" : "middle";
      ctx.save();
      ctx.fillStyle = bgColor.current;
      ctx.fillRect(0, 0, width, height);
      ctx.restore();

      const [markers, lenForMarkers] = markersAndLength;
      if (markers.length > 0) {
        const {
          MAJOR_TICK_POS,
          MINOR_TICK_POS,
          LABEL_POS,
          LABEL_ADJUSTMENT: LABEL_MARGIN,
        } = markerPos;

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
              ctx.fillText(label, pxPosition + LABEL_MARGIN, LABEL_POS);
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
              ctx.fillText(label, LABEL_POS, pxPosition + LABEL_MARGIN);
              ctx.moveTo(MAJOR_TICK_POS, pxPosition);
            } else {
              ctx.moveTo(MINOR_TICK_POS, pxPosition);
            }
            ctx.lineTo(0, pxPosition);
            ctx.closePath();
            ctx.stroke();
          });
        }
      }
    }, [
      devicePixelRatio,
      width,
      height,
      markersAndLength,
      markerPos,
      direction,
      axisPadding,
      shiftWhenResize,
      correctMarkerPos,
    ]);

    const drawRef = useRef(draw);
    const lastTimestampRef = useRef<number>(-1);
    useEffect(() => {
      drawRef.current = draw;
      // Request a redraw only when the draw function or its dependencies change
      const animationFrameId = requestAnimationFrame((timestamp) => {
        if (timestamp === lastTimestampRef.current) return;
        lastTimestampRef.current = timestamp;
        // Ensure drawRef.current exists and call it
        if (drawRef.current) drawRef.current();
      });

      // Cleanup function to cancel the frame if the component unmounts
      // or if dependencies change again before the frame executes
      return () => cancelAnimationFrame(animationFrameId);
    }, [draw]);

    const imperativeInstanceRef = useRef<AxisCanvasHandleElement>({
      getBoundingClientRect: () => canvasElem.current?.getBoundingClientRect() ?? null,
    });
    useImperativeHandle(ref, () => imperativeInstanceRef.current, []);

    return (
      <>
        <canvas
          className={`AxisCanvas ${styles[className]}`}
          ref={canvasElemCallback}
          style={{ width, height }}
          onContextMenu={(e) => {
            e.preventDefault();
            BackendAPI.showAxisContextMenu(className, id);
          }}
          onClick={onClick}
          onMouseEnter={onTooltipMouseMove}
          onMouseMove={onTooltipMouseMove}
          onMouseLeave={onTooltipMouseLeave}
        />
        {tooltip
          ? createPortal(
              <span
                ref={tooltipElemRef}
                className={styles.axisTooltip}
                style={{ left: tooltip.left, top: tooltip.top }}
              >
                {tooltip.text}
              </span>,
              document.body,
            )
          : null}
      </>
    );
  },
);
AxisCanvas.displayName = "AxisCanvas";

export default React.memo(AxisCanvas);
