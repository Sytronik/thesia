import React, {useCallback, useEffect, useRef} from "react";
import useEvent from "react-use-event-hook";
import {TINY_MARGIN} from "renderer/prototypes/constants/tracks";
import styles from "./Locator.module.scss";

type LocatorProps = {
  locatorStyle: "selection" | "playhead";
  getHeight: () => number;
  getBoundingLeftWidth: () => [number, number] | null;
  calcLocatorPos: () => number;
  onMouseDown?: (e: React.MouseEvent) => void;
};

function Locator(props: LocatorProps) {
  const {locatorStyle, getHeight, getBoundingLeftWidth, calcLocatorPos, onMouseDown} = props;
  const locatorElem = useRef<HTMLCanvasElement | null>(null);
  const locatorCtxRef = useRef<CanvasRenderingContext2D | null>(null);
  const locatorElemCallback = useCallback((node: HTMLCanvasElement | null) => {
    locatorElem.current = node;
    locatorCtxRef.current = node?.getContext("2d") ?? null;
  }, []);
  const requestRef = useRef<number>(0);

  const lineWidth = locatorStyle === "selection" ? 2 : 1;
  const lineOffset = lineWidth % 2 === 0 ? 0 : 0.5;

  const drawLine = useEvent(
    (ctx: CanvasRenderingContext2D, drawPos: number, lineHeight: number) => {
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
      ctx.moveTo(drawPos + lineOffset, TINY_MARGIN * 2);
      ctx.lineTo(drawPos + lineOffset, lineHeight);
      ctx.stroke();
    },
  );

  const draw = useEvent(() => {
    const leftWidth = getBoundingLeftWidth();
    if (leftWidth !== null && locatorElem.current !== null) {
      const [left, width] = leftWidth;
      const locatorPos = calcLocatorPos();

      if (
        locatorPos <= -lineOffset - lineWidth / 2 ||
        locatorPos >= width + lineWidth / 2 - lineOffset
      ) {
        if (locatorElem.current.style.visibility !== "hidden")
          locatorElem.current.style.visibility = "hidden";
      } else {
        const locatorElemPos = Math.floor(locatorPos) - 1;
        const drawPos = locatorPos - locatorElemPos;
        const lineHeight = getHeight();
        if (locatorElem.current.style.visibility !== "") locatorElem.current.style.visibility = "";
        if (locatorElem.current.style.left !== `${locatorElemPos + left}px`)
          locatorElem.current.style.left = `${locatorElemPos + left}px`;
        if (locatorElem.current.style.height !== `${lineHeight}px`)
          locatorElem.current.style.height = `${lineHeight}px`;
        locatorElem.current.width = 5 * devicePixelRatio;
        locatorElem.current.height = lineHeight * devicePixelRatio;
        const ctx = locatorCtxRef.current;
        if (ctx !== null) drawLine(ctx, drawPos, lineHeight);
      }
    }
    requestRef.current = requestAnimationFrame(draw);
  });

  useEffect(() => {
    requestRef.current = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(requestRef.current);
  }, [draw]);

  return (
    <canvas
      ref={locatorElemCallback}
      className={styles.locator}
      onMouseDown={onMouseDown}
      style={onMouseDown ? {cursor: "col-resize"} : {pointerEvents: "none"}}
    />
  );
}

export default Locator;
