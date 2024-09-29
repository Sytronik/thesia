import React, {forwardRef, useCallback, useEffect, useImperativeHandle, useRef} from "react";
import useEvent from "react-use-event-hook";
import {areDOMRectsEqual} from "renderer/utils/arrayUtils";
import styles from "./Locator.module.scss";

type LocatorProps = {
  locatorStyle: "selection" | "playhead";
  getTopBottom: () => [number, number];
  getBoundingLeftWidth: () => [number, number] | null;
  calcLocatorPos: () => number;
  onMouseDown?: (e: React.MouseEvent) => void;
  zIndex?: number;
};

const Locator = forwardRef((props: LocatorProps, ref) => {
  const {locatorStyle, getTopBottom, getBoundingLeftWidth, calcLocatorPos, onMouseDown, zIndex} =
    props;
  const locatorElem = useRef<HTMLCanvasElement | null>(null);
  const locatorCtxRef = useRef<CanvasRenderingContext2D | null>(null);
  const locatorElemCallback = useCallback((node: HTMLCanvasElement | null) => {
    locatorElem.current = node;
    locatorCtxRef.current = node?.getContext("2d") ?? null;
  }, []);
  const requestRef = useRef<number>(0);
  const prevLocatorPos = useRef<number>(-1);
  const prevLeftWidth = useRef<[number, number]>([-1, -1]);
  const prevBoundingRect = useRef<DOMRect>(new DOMRect());

  const lineWidth = locatorStyle === "selection" ? 2 : 1;
  const lineOffset = lineWidth % 2 === 0 ? 0 : 0.5;
  const moveElem = onMouseDown !== undefined;

  const imperativeHandleRef = useRef<LocatorHandleElement>({
    enableInteraction: () => {
      if (locatorElem.current) locatorElem.current.style.pointerEvents = "auto";
    },
    disableInteraction: () => {
      if (locatorElem.current) locatorElem.current.style.pointerEvents = "none";
    },
  });
  useImperativeHandle(ref, () => imperativeHandleRef.current, []);

  const drawLine = useEvent(
    (ctx: CanvasRenderingContext2D, drawPos: number, lineTop: number, lineBottom: number) => {
      ctx.scale(devicePixelRatio, devicePixelRatio);
      ctx.lineWidth = lineWidth;
      switch (locatorStyle) {
        case "selection":
          ctx.strokeStyle = "#999999";
          ctx.beginPath();
          ctx.setLineDash([5, 5]);
          break;
        case "playhead":
          ctx.strokeStyle = "#DDDDDD";
          ctx.beginPath();
          break;
        default:
          break;
      }
      ctx.moveTo(drawPos + lineOffset, lineTop);
      ctx.lineTo(drawPos + lineOffset, lineBottom);
      ctx.stroke();
    },
  );

  const draw = useEvent(() => {
    const locatorPos = calcLocatorPos();
    const leftWidth = getBoundingLeftWidth();

    if (locatorElem.current !== null) {
      const rect = locatorElem.current.getBoundingClientRect();
      if (
        leftWidth !== null &&
        (Math.abs(locatorPos - prevLocatorPos.current) > 1e-3 ||
          leftWidth.some((v, i) => Math.abs(v - prevLeftWidth.current[i]) > 1e-1) ||
          !areDOMRectsEqual(rect, prevBoundingRect.current))
      ) {
        const [left, width] = leftWidth;

        if (
          locatorPos <= -lineOffset - lineWidth / 2 ||
          locatorPos >= width + lineWidth / 2 - lineOffset
        ) {
          if (locatorElem.current.style.visibility !== "hidden")
            locatorElem.current.style.visibility = "hidden";
        } else {
          const locatorElemPos = moveElem ? Math.floor(locatorPos) - 1 : 0;
          const drawPos = locatorPos - locatorElemPos;
          const [lineTop, lineBottom] = getTopBottom();
          if (locatorElem.current.style.visibility !== "")
            locatorElem.current.style.visibility = "";
          const styleLeft = `${locatorElemPos + left}px`;
          if (locatorElem.current.style.left !== styleLeft)
            locatorElem.current.style.left = styleLeft;
          locatorElem.current.width = rect.width * devicePixelRatio;
          locatorElem.current.height = rect.height * devicePixelRatio;
          const ctx = locatorCtxRef.current;
          if (ctx !== null) drawLine(ctx, drawPos, lineTop, lineBottom);
        }
      }
      prevBoundingRect.current = rect;
    }
    prevLocatorPos.current = locatorPos;
    if (leftWidth) prevLeftWidth.current = leftWidth;
    requestRef.current = requestAnimationFrame(draw);
  });

  useEffect(() => {
    requestRef.current = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(requestRef.current);
  }, [draw]);

  return (
    <canvas
      ref={locatorElemCallback}
      className={onMouseDown ? styles.interactiveLocator : styles.nonInteractiveLocator}
      onMouseDown={onMouseDown}
      style={{zIndex}}
    />
  );
});

Locator.displayName = "Locator";

export default Locator;
