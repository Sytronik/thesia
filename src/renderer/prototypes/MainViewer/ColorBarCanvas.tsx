import React, {forwardRef, useMemo, useRef, useImperativeHandle} from "react";
import {chunk} from "renderer/utils/arrayUtils";
import {COLORBAR_CANVAS_WIDTH, COLORBAR_COLORS_COUNT} from "renderer/prototypes/constants";
import NativeAPI from "../../api";
import styles from "./ColorBarCanvas.scss";

type ColorBarCanvasProps = {
  width: number;
  height: number;
};

const COLORBAR_CENTER = COLORBAR_CANVAS_WIDTH / 2;

const ColorBarCanvas = forwardRef((props: ColorBarCanvasProps, ref) => {
  const {width, height} = props;
  const canvasElem = useRef<HTMLCanvasElement>(null);

  const colorBarGradientBuf = useMemo(() => NativeAPI.getColorMap(), []);

  useImperativeHandle(ref, () => ({
    draw: async () => {
      if (!(colorBarGradientBuf.byteLength === COLORBAR_COLORS_COUNT * 3)) {
        return;
      }

      const ctx = canvasElem?.current?.getContext("2d");

      if (!ctx) {
        return;
      }

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
    },
  }));

  return (
    <>
      <canvas className={styles.ColorBarCanvas} ref={canvasElem} height={height} width={width} />
    </>
  );
});
ColorBarCanvas.displayName = "ColorBarCanvas";

export default ColorBarCanvas;
