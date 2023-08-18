import React, {forwardRef, useRef, useImperativeHandle, useEffect} from "react";
import styles from "./ImgCanvas.scss";

type ImgCanvasProps = {
  width: number;
  height: number;
  pixelRatio: number;
};

const ImgCanvas = forwardRef((props: ImgCanvasProps, ref) => {
  const {width, height, pixelRatio} = props;
  const canvasElem = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    if (!canvasElem.current) return;

    canvasElem.current.width = width * pixelRatio;
    canvasElem.current.height = height * pixelRatio;
  }, [width, height, pixelRatio]);

  useImperativeHandle(
    ref,
    () => ({
      draw: async (buf: Buffer) => {
        const bitmapWidth = width * pixelRatio;
        const bitmapHeight = height * pixelRatio;
        if (!(buf && buf.byteLength === 4 * bitmapWidth * bitmapHeight)) {
          return;
        }

        const ctx = canvasElem.current?.getContext("bitmaprenderer");
        if (!ctx) return;

        const imdata = new ImageData(new Uint8ClampedArray(buf), bitmapWidth, bitmapHeight);
        const imbmp = await createImageBitmap(imdata);
        ctx.transferFromImageBitmap(imbmp);
      },
    }),
    [width, height, pixelRatio],
  );

  return <canvas className={styles.ImgCanvas} ref={canvasElem} style={{width, height}} />;
});
ImgCanvas.displayName = "ImgCanvas";

export default ImgCanvas;
