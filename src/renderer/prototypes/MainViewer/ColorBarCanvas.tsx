import React, {useMemo, useRef, useCallback, useEffect} from "react";
import useStore from "renderer/hooks/useStore";
import {chunk} from "renderer/utils/arrayUtils";
import {COLORBAR_CANVAS_WIDTH} from "renderer/prototypes/constants/tracks";
import BackendAPI from "../../api";
import styles from "./ColorBarCanvas.module.scss";

type ColorBarCanvasProps = {
  width: number;
  height: number;
};

const COLORBAR_CENTER = COLORBAR_CANVAS_WIDTH / 2;

function ColorBarCanvas(props: ColorBarCanvasProps) {
  const {width, height} = props;
  const devicePixelRatio = useStore().getDPR();
  const canvasElem = useRef<HTMLCanvasElement>(null);
  const ctxRef = useRef<CanvasRenderingContext2D | null>(null);
  const requestRef = useRef<number | null>(null);

  const colorBarGradientBuf = useMemo(() => BackendAPI.getColorMap(), []);

  const draw = useCallback(() => {
    requestRef.current = null;
    if (colorBarGradientBuf.byteLength % 3 !== 0) return;

    const ctx = ctxRef.current;
    if (!ctx) return;

    const gradientColors = new Uint8Array(colorBarGradientBuf);
    const gradientColorMap = chunk([...gradientColors], 3).reverse();
    const colorGradient = ctx.createLinearGradient(COLORBAR_CENTER, 0, COLORBAR_CENTER, height);

    gradientColorMap.forEach((color, idx) => {
      const [r, g, b] = color;
      colorGradient.addColorStop(
        (1 / (gradientColorMap.length - 1)) * idx,
        `rgba(${r}, ${g}, ${b}, 1)`,
      );
    });

    ctx.fillStyle = colorGradient;
    ctx.fillRect(0, 0, COLORBAR_CANVAS_WIDTH, height);
    requestRef.current = requestAnimationFrame(draw);
  }, [colorBarGradientBuf, height]);

  useEffect(() => {
    if (!canvasElem.current) return;
    canvasElem.current.width = width * devicePixelRatio;
    canvasElem.current.height = height * devicePixelRatio;

    ctxRef.current = canvasElem.current.getContext("2d", {alpha: false, desynchronized: true});
    ctxRef.current?.scale(devicePixelRatio, devicePixelRatio);
    draw();
  }, [draw, width, height, devicePixelRatio]);

  return <canvas className={styles.ColorBarCanvas} ref={canvasElem} style={{width, height}} />;
}

export default ColorBarCanvas;
