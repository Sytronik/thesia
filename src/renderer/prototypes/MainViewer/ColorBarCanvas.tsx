import React, {useMemo, useRef, useCallback, useEffect, useContext} from "react";
import {chunk} from "renderer/utils/arrayUtils";
import {COLORBAR_CANVAS_WIDTH, COLORBAR_COLORS_COUNT} from "renderer/prototypes/constants";
import {DevicePixelRatioContext} from "renderer/contexts";
import BackendAPI from "../../api";
import styles from "./ColorBarCanvas.scss";

type ColorBarCanvasProps = {
  width: number;
  height: number;
};

const COLORBAR_CENTER = COLORBAR_CANVAS_WIDTH / 2;

function ColorBarCanvas(props: ColorBarCanvasProps) {
  const {width, height} = props;
  const devicePixelRatio = useContext(DevicePixelRatioContext);
  const canvasElem = useRef<HTMLCanvasElement>(null);
  const ctxRef = useRef<CanvasRenderingContext2D | null>(null);
  const requestRef = useRef<number | null>(null);
  const prevHeightRef = useRef<number>(0);

  const colorBarGradientBuf = useMemo(() => BackendAPI.getColorMap(), []);

  useEffect(() => {
    if (!canvasElem.current) return;
    canvasElem.current.width = width * devicePixelRatio;
    canvasElem.current.height = height * devicePixelRatio;

    ctxRef.current = canvasElem.current.getContext("2d", {alpha: false, desynchronized: true});
    ctxRef.current?.scale(devicePixelRatio, devicePixelRatio);
  }, [width, height, devicePixelRatio]);

  const draw = useCallback(() => {
    requestRef.current = null;
    if (!(colorBarGradientBuf.byteLength === COLORBAR_COLORS_COUNT * 3)) return;

    const ctx = ctxRef.current;
    if (!ctx) {
      prevHeightRef.current = 0;
      return;
    }

    if (prevHeightRef.current === height) return;

    const gradientColors = new Uint8Array(colorBarGradientBuf);
    const gradientColorMap = chunk([...gradientColors], 3).reverse();
    const colorGradient = ctx.createLinearGradient(COLORBAR_CENTER, 0, COLORBAR_CENTER, height);

    gradientColorMap.forEach((color, idx) => {
      const [r, g, b] = color;
      colorGradient.addColorStop(
        (1 / (COLORBAR_COLORS_COUNT - 1)) * idx,
        `rgba(${r}, ${g}, ${b}, 1)`,
      );
    });

    ctx.fillStyle = colorGradient;
    ctx.fillRect(0, 0, COLORBAR_CANVAS_WIDTH, height);
    prevHeightRef.current = height;
    requestRef.current = requestAnimationFrame(draw);
  }, [colorBarGradientBuf, height]);

  useEffect(() => {
    requestRef.current = requestAnimationFrame(draw);

    return () => {
      if (requestRef.current !== null) cancelAnimationFrame(requestRef.current);
    };
  }, [draw, devicePixelRatio]);

  return <canvas className={styles.ColorBarCanvas} ref={canvasElem} style={{width, height}} />;
}

export default ColorBarCanvas;
