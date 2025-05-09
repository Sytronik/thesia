import React, {useMemo, useRef, useCallback, useEffect, useContext} from "react";
import {chunk} from "renderer/utils/arrayUtils";
import {COLORBAR_CANVAS_WIDTH} from "renderer/prototypes/constants/tracks";
import {DevicePixelRatioContext} from "renderer/contexts";
import BackendAPI from "../../api";
import styles from "./ColorBarCanvas.module.scss";

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

  const colorBarGradientBuf = useMemo(
    () => chunk([...new Uint8Array(BackendAPI.getColorMap())], 3).reverse(),
    [],
  );

  const draw = useCallback(() => {
    const ctx = ctxRef.current;
    if (!ctx) return;

    const colorGradient = ctx.createLinearGradient(COLORBAR_CENTER, 0, COLORBAR_CENTER, height);

    colorBarGradientBuf.forEach((color, idx) => {
      const [r, g, b] = color;
      colorGradient.addColorStop(
        (1 / (colorBarGradientBuf.length - 1)) * idx,
        `rgba(${r}, ${g}, ${b}, 1)`,
      );
    });

    ctx.fillStyle = colorGradient;
    ctx.fillRect(0, 0, COLORBAR_CANVAS_WIDTH, height);
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

export default React.memo(ColorBarCanvas);
