import React, {forwardRef, useRef, useImperativeHandle} from "react";

const ImgCanvas = forwardRef(({width, height}, ref) => {
  const canvasElem = useRef(null);

  useImperativeHandle(ref, () => ({
    draw: async (buf) => {
      if (!(buf && buf.byteLength === 4 * width * height)) {
        return;
      }
      const ctx = canvasElem.current.getContext("bitmaprenderer");
      const imdata = new ImageData(new Uint8ClampedArray(buf), width, height);
      const imbmp = await createImageBitmap(imdata);
      ctx.transferFromImageBitmap(imbmp);
    },
  }));

  return (
    <>
      <canvas className="img" ref={canvasElem} height={height} width={width - 48} /> {/* TEMP */}
    </>
  );
});

export default ImgCanvas;
