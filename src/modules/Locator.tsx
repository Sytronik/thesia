import React, { forwardRef, useCallback, useEffect, useImperativeHandle, useRef } from "react";
import useEvent from "react-use-event-hook";
import { areDOMRectsEqual } from "src/utils/arrayUtils";
import styles from "./Locator.module.scss";

type LocatorProps = {
  locatorStyle: "selection" | "playhead";
  getLineTopBottom: () => [number, number];
  getBoundingLeftWidthTop: () => [number, number, number] | null;
  calcLocatorPos: () => number;
  zIndex?: number;
};

const Locator = forwardRef((props: LocatorProps, ref) => {
  const { locatorStyle, getLineTopBottom, getBoundingLeftWidthTop, calcLocatorPos, zIndex } = props;
  const locatorElem = useRef<HTMLCanvasElement | null>(null);
  const locatorCtxRef = useRef<CanvasRenderingContext2D | null>(null);
  const locatorElemCallback = useCallback((node: HTMLCanvasElement | null) => {
    locatorElem.current = node;
    locatorCtxRef.current = node?.getContext("2d") ?? null;
  }, []);
  const requestRef = useRef<number>(0);
  const prevLocatorPos = useRef<number>(-1);
  const prevLeftWidthTop = useRef<[number, number, number]>([-1, -1, -1]);
  const prevBoundingRect = useRef<DOMRect>(new DOMRect());

  const lineWidth = locatorStyle === "selection" ? 2 : 1;
  const lineOffset = lineWidth % 2 === 0 ? 0 : 0.5;

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

  const drawRef = useRef<(() => void) | null>(null);
  const draw = useEvent(() => {
    const locatorPos = calcLocatorPos();
    const leftWidthTop = getBoundingLeftWidthTop();

    if (locatorElem.current !== null) {
      const rect = locatorElem.current.getBoundingClientRect();
      if (
        leftWidthTop !== null &&
        (Math.abs(locatorPos - prevLocatorPos.current) > 1e-3 ||
          leftWidthTop.some((v, i) => Math.abs(v - prevLeftWidthTop.current[i]) > 1e-1) ||
          !areDOMRectsEqual(rect, prevBoundingRect.current))
      ) {
        const [left, width, top] = leftWidthTop;

        if (
          locatorPos <= -lineOffset - lineWidth / 2 ||
          locatorPos >= width + lineWidth / 2 - lineOffset
        ) {
          if (locatorElem.current.style.visibility !== "hidden")
            locatorElem.current.style.visibility = "hidden";
        } else {
          const [lineTop, lineBottom] = getLineTopBottom();
          if (locatorElem.current.style.visibility !== "")
            locatorElem.current.style.visibility = "";
          const leftOffset = Math.floor(lineWidth / 2);
          const styleLeft = `${left - leftOffset}px`;
          if (locatorElem.current.style.left !== styleLeft)
            locatorElem.current.style.left = styleLeft;
          const styleTop = `${top}px`;
          if (locatorElem.current.style.top !== styleTop) locatorElem.current.style.top = styleTop;
          locatorElem.current.width = rect.width * devicePixelRatio;
          locatorElem.current.height = rect.height * devicePixelRatio;
          const ctx = locatorCtxRef.current;
          if (ctx !== null) drawLine(ctx, locatorPos + leftOffset, lineTop, lineBottom);
        }
      }
      prevBoundingRect.current = rect;
    }
    prevLocatorPos.current = locatorPos;
    if (leftWidthTop) prevLeftWidthTop.current = leftWidthTop;
    if (drawRef.current) requestRef.current = requestAnimationFrame(drawRef.current);
  });

  useEffect(() => {
    drawRef.current = draw;
    requestRef.current = requestAnimationFrame(drawRef.current);
    return () => cancelAnimationFrame(requestRef.current);
  }, [draw]);

  const imperativeHandleRef = useRef<LocatorHandleElement>({
    isOnLocator: (clientX: number) => {
      const rect = locatorElem.current?.getBoundingClientRect() ?? null;
      if (rect === null) return false;
      const [clientLeft, width] = prevLeftWidthTop.current;
      const clientRight = clientLeft + width;
      const locatorClientX = rect.left + prevLocatorPos.current;
      const margin = lineWidth / 2 + 2;
      return (
        Math.max(locatorClientX - margin, clientLeft) <= clientX &&
        clientX < Math.min(locatorClientX + margin, clientRight)
      );
    },
  });
  useImperativeHandle(ref, () => imperativeHandleRef.current, []);

  return <canvas ref={locatorElemCallback} className={styles.locator} style={{ zIndex }} />;
});

Locator.displayName = "Locator";

export default React.memo(Locator);
