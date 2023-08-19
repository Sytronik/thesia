import React, {useMemo, useRef, useCallback, useEffect} from "react";
import {chunk} from "renderer/utils/arrayUtils";
import {COLORBAR_CANVAS_WIDTH, COLORBAR_COLORS_COUNT} from "renderer/prototypes/constants";
import NativeAPI from "../../api";
import styles from "./ColorBarCanvas.scss";

type ColorBarCanvasProps = {
  width: number;
  height: number;
  pixelRatio: number;
};

const COLORBAR_CENTER = COLORBAR_CANVAS_WIDTH / 2;

function ColorBarCanvas(props: ColorBarCanvasProps) {
  const {width, height, pixelRatio} = props;
  const canvasElem = useRef<HTMLCanvasElement>(null);
  const ctxRef = useRef<CanvasRenderingContext2D | null>(null);

  const colorBarGradientBuf = useMemo(() => NativeAPI.getColorMap(), []);

  useEffect(() => {
    if (!canvasElem.current) return;
    canvasElem.current.width = width * pixelRatio;
    canvasElem.current.height = height * pixelRatio;

    ctxRef.current = canvasElem.current.getContext("2d");
    ctxRef.current?.scale(pixelRatio, pixelRatio);
  }, [width, height, pixelRatio]);

  const draw = useCallback(() => {
    if (!(colorBarGradientBuf.byteLength === COLORBAR_COLORS_COUNT * 3)) {
      return;
    }

    const ctx = ctxRef.current;
    if (!ctx) return;

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
  }, [colorBarGradientBuf, height]);

  useEffect(draw, [draw]);

  return <canvas className={styles.ColorBarCanvas} ref={canvasElem} style={{width, height}} />;
}

export default ColorBarCanvas;
