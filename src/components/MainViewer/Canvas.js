import React, {forwardRef, useRef, useImperativeHandle} from "react";

const Canvas = forwardRef(({width, height}, ref) => {
  const canvasElem = useRef(null);
  const timeRef = useRef(0);

  useImperativeHandle(ref, () => ({
    draw: (bufs) => {
      const [bufSpec, bufWav] = bufs;
      if (!bufSpec && !bufWav) {
        return;
      }
      if (
        !(bufSpec && bufSpec.byteLength === 4 * width * height) &&
        !(bufWav && bufWav.byteLength === 4 * width * height)
      ) {
        return;
      }
      // console.log(canvas.current);
      // const ctx = canvas.current.getContext("bitmaprenderer");
      // const imdata = new ImageData(new Uint8ClampedArray(bufSpec), width, height);
      // const imbmp = await createImageBitmap(imdata);
      // ctx.transferFromImageBitmap(imbmp);

      requestAnimationFrame(async (timestamp) => {
        if (timeRef.current === timestamp) return;
        timeRef.current = timestamp;
        const imgSpec = new ImageData(new Uint8ClampedArray(bufSpec), width, height);
        const imgWav = new ImageData(new Uint8ClampedArray(bufWav), width, height);
        const offscreen = new OffscreenCanvas(width, height);
        const ctx = offscreen.getContext("2d");
        ctx.clearRect(0, 0, width, height);
        const promiseBmpSpec = createImageBitmap(imgSpec);
        const promiseBmpWav = createImageBitmap(imgWav);
        const [bmpSpec, bmpWav] = await Promise.all([promiseBmpSpec, promiseBmpWav]);
        ctx.drawImage(bmpSpec, 0, 0);
        ctx.fillStyle = "rgba(0, 0, 0, 0.5)";
        ctx.fillRect(0, 0, offscreen.width, offscreen.height);
        ctx.drawImage(bmpWav, 0, 0);
        canvasElem.current
          .getContext("bitmaprenderer")
          .transferFromImageBitmap(offscreen.transferToImageBitmap());
        // imgWav.data.forEach((d, i, a) => {
        //   if ((i + 1) % 4) {
        //     a[i] = 128;
        //   }
        // });
        // ctx.putImageData(imgWav, 0, 0);
      });
    },
  }));

  return (
    <>
      <canvas ref={canvasElem} height={height} width={width} />
    </>
  );
});

export default Canvas;
