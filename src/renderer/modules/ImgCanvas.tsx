import React, {forwardRef, useRef, useImperativeHandle} from "react";
import styles from "./ImgCanvas.scss";

type ImgCanvasProps = {
  width: number;
  height: number;
};

const ImgCanvas = forwardRef((props: ImgCanvasProps, ref) => {
  const {width, height} = props;
  const canvasElem = useRef<HTMLCanvasElement>(null);

  useImperativeHandle(ref, () => ({
    draw: async (buf: ArrayBuffer) => {
      if (!(buf && buf.byteLength === 4 * width * height)) {
        return;
      }

      const ctx = canvasElem?.current?.getContext("bitmaprenderer");

      if (!ctx) {
        return;
      }

      const imdata = new ImageData(new Uint8ClampedArray(buf), width, height);
      const imbmp = await createImageBitmap(imdata);
      ctx.transferFromImageBitmap(imbmp);
    },
  }));

  return (
    <>
      <canvas
        className={styles.ImgCanvas}
        ref={canvasElem}
        height={height}
        width={width}
        style={{width, height}}
      />{" "}
      {/* TEMP */}
    </>
  );
});
ImgCanvas.displayName = "ImgCanvas";

export default ImgCanvas;
