import {WAV_BORDER_COLOR} from "renderer/prototypes/constants/colors";
import {
  WAV_BORDER_WIDTH,
  WAV_LINE_WIDTH_FACTOR,
  WAV_MARGIN_PX,
} from "renderer/prototypes/constants/tracks";

export type WavDrawingOptions = {
  startPx: number; // canvas pixels
  pxPerPoints: number; // canvas pixels per point
  height: number; // canvas pixels
  offsetY?: number; // canvas pixels

  // to set line style
  scale: number;
  devicePixelRatio: number;
  color: string;

  clipValues?: [number, number] | null;
  needBorder?: boolean;
};

export type WavDrawingOptions2 = {
  startSec: number;
  pxPerSec: number; // css pixels per second
  ampRange: [number, number];

  color: string;

  scale: number;
  devicePixelRatio: number;
  offsetY?: number; // canvas pixels (TODO: change to css pixels?)

  clipValues?: [number, number] | null;
  needBorderForEnvelope?: boolean;
  needBorderForLine?: boolean;
  doClear?: boolean;
};

const clipFn = (clipValues: [number, number] | null) => {
  if (!clipValues) return (v: number) => v;
  const [min, max] = clipValues;
  return (v: number) => Math.min(Math.max(v, min), max);
};

const setLinePath = (
  ctx: CanvasRenderingContext2D,
  points: Float32Array,
  startPx: number,
  pxPerPoints: number,
  height: number,
  offsetY: number = 0,
  clipValues: [number, number] | null = null,
) => {
  const clip = clipFn(clipValues);
  const relYtoY = (v: number) => clip(v) * height + offsetY;
  ctx.moveTo(startPx, relYtoY(points[0]));
  ctx.beginPath();
  points.forEach((v, i) => {
    if (i === 0) return;
    ctx.lineTo(startPx + i * pxPerPoints, relYtoY(v));
  });
};

export const drawWavLine = (
  ctx: CanvasRenderingContext2D,
  wavLine: Float32Array,
  options: WavDrawingOptions,
  lineWidthFactor: number = WAV_LINE_WIDTH_FACTOR,
) => {
  ctx.lineCap = "round";
  ctx.lineJoin = "round";

  // border
  if (options.needBorder) {
    ctx.strokeStyle = WAV_BORDER_COLOR;
    ctx.lineWidth =
      lineWidthFactor * options.scale + 2 * WAV_BORDER_WIDTH * options.devicePixelRatio;
    setLinePath(
      ctx,
      wavLine,
      options.startPx,
      options.pxPerPoints,
      options.height,
      options.offsetY,
      options.clipValues,
    );
    ctx.stroke();
  }

  // line
  ctx.strokeStyle = options.color;
  ctx.lineWidth = lineWidthFactor * options.scale;
  setLinePath(
    ctx,
    wavLine,
    options.startPx,
    options.pxPerPoints,
    options.height,
    options.offsetY,
    options.clipValues,
  );
  ctx.stroke();
};

const setEnvelopePath = (
  ctx: CanvasRenderingContext2D,
  topEnvelope: Float32Array,
  bottomEnvelope: Float32Array,
  startPx: number,
  pxPerPoints: number,
  height: number,
  offsetY: number = 0,
  clipValues: [number, number] | null = null,
  strokeWidth: number = 0,
) => {
  const clip = clipFn(clipValues);
  const relYtoY = (v: number) => clip(v) * height + offsetY;
  ctx.moveTo(startPx, relYtoY(topEnvelope[0]));
  ctx.beginPath();
  for (let i = 1; i < topEnvelope.length; i += 1) {
    ctx.lineTo(startPx + i * pxPerPoints, relYtoY(topEnvelope[i]) - strokeWidth / 2);
  }
  for (let i = bottomEnvelope.length - 1; i >= 0; i -= 1) {
    ctx.lineTo(startPx + i * pxPerPoints, relYtoY(bottomEnvelope[i]) + strokeWidth / 2);
  }
  ctx.closePath();
};

export const drawWavEnvelope = (
  ctx: CanvasRenderingContext2D,
  topEnvelope: Float32Array,
  bottomEnvelope: Float32Array,
  options: WavDrawingOptions,
) => {
  if (options.needBorder) {
    ctx.lineCap = "round";
    ctx.lineJoin = "round";
    ctx.strokeStyle = WAV_BORDER_COLOR;
    ctx.lineWidth = WAV_BORDER_WIDTH * options.devicePixelRatio;
    setEnvelopePath(
      ctx,
      topEnvelope,
      bottomEnvelope,
      options.startPx,
      options.pxPerPoints,
      options.height,
      options.offsetY,
      options.clipValues,
      ctx.lineWidth,
    );
    ctx.stroke();
  }

  // fill
  ctx.fillStyle = options.color;
  setEnvelopePath(
    ctx,
    topEnvelope,
    bottomEnvelope,
    options.startPx,
    options.pxPerPoints,
    options.height,
    options.offsetY,
    options.clipValues,
  );
  ctx.fill();
};

const envelopeToPath = (
  topEnvelope: [number, number][],
  bottomEnvelope: [number, number][],
  strokeWidth: number,
) => {
  const path = new Path2D();
  const halfStrokeWidth = strokeWidth / 2;
  path.moveTo(topEnvelope[0][0], topEnvelope[0][1]);
  topEnvelope.forEach(([x, y], i) => {
    if (i === 0) return;
    path.lineTo(x, y - halfStrokeWidth);
  });
  bottomEnvelope.reverse().forEach(([x, y]) => {
    path.lineTo(x, y + halfStrokeWidth);
  });
  path.closePath();
  return path;
};

export const drawWav = (
  ctx: CanvasRenderingContext2D,
  wav: number[],
  sr: number,
  options: WavDrawingOptions2,
) => {
  const {
    startSec,
    ampRange,
    color,
    scale,
    devicePixelRatio,
    offsetY = 0,
    clipValues = null,
    needBorderForEnvelope = true,
    needBorderForLine = true,
    doClear = true,
  } = options;

  const width = ctx.canvas.width * scale;
  const height = ctx.canvas.height * scale;
  const pxPerSec = options.pxPerSec * scale * devicePixelRatio;
  const strokeWidth = WAV_LINE_WIDTH_FACTOR * scale * devicePixelRatio;

  const offsetX = -startSec * pxPerSec;
  const idxToX = (idx: number) => (idx * pxPerSec) / sr + offsetX;
  const floorX = (x: number) => Math.floor((x - offsetX) / scale) * scale + offsetX;

  const ampRangeScale = Math.max(ampRange[1] - ampRange[0], 1e-8);
  const clip = clipFn(clipValues);
  const wavToY = (v: number) => ((ampRange[1] - clip(v)) * height) / ampRangeScale + offsetY;

  const marginSamples = (WAV_MARGIN_PX / pxPerSec) * sr;
  const iStart = Math.floor(startSec * sr - marginSamples);
  const iEnd = Math.ceil((startSec + width / pxPerSec) * sr + marginSamples);

  let linePath = null;
  let topEnvelope: [number, number][] | null = null;
  let bottomEnvelope: [number, number][] | null = null;
  const envelopePaths: Path2D[] = [];

  let i = iStart;
  let iPrev = i;
  while (i < iEnd) {
    const x = idxToX(i);
    const y = wavToY(wav[i]);
    if (pxPerSec < sr) {
      // downsampling
      const xFloor = floorX(x);
      const xMid = xFloor + scale / 2;
      let top = y;
      let bottom = y;
      let i2 = iPrev; // context size == scale, iPrev <= i2 < iNext <= iEnd
      let iNext = iEnd;
      while (i2 < iEnd) {
        // find top and bottom (min and max)
        const x2 = idxToX(i2);
        const x2Floor = floorX(x2);
        if (x2Floor > xFloor + scale) break;
        if (x2Floor > xFloor && iNext === iEnd) iNext = i2;
        const y2 = wavToY(wav[i2]);
        if (y2 < top) top = y2;
        if (y2 > bottom) bottom = y2;
        i2 += 1;
      }
      if (bottom - top > strokeWidth / 2) {
        // need to draw envelope
        if (!topEnvelope || !bottomEnvelope) {
          // new envelope starts
          const prevY = wavToY(wav[i - 1]); // start point of the envelope
          topEnvelope = [[xFloor, prevY]];
          bottomEnvelope = [[xFloor, prevY]];
          if (!linePath) {
            linePath = new Path2D();
            linePath.moveTo(xMid, y);
          } else {
            linePath.lineTo(xMid, y);
          }
        }
        // continue the envelope
        topEnvelope.push([xMid, top]);
        bottomEnvelope.push([xMid, bottom]);
        linePath?.lineTo(xMid, (top + bottom) / 2);
      } else {
        // no need to draw envelope
        if (topEnvelope && bottomEnvelope) {
          // the recent envelope is finished
          topEnvelope.push([xFloor, y]);
          bottomEnvelope.push([xFloor, y]);
          // add the envelope to the paths
          envelopePaths.push(envelopeToPath(topEnvelope, bottomEnvelope, strokeWidth));
          topEnvelope = null;
          bottomEnvelope = null;
          linePath?.lineTo(xMid - 1, wavToY(wav[i - 1]));
        }
        // continue the line
        if (!linePath) {
          linePath = new Path2D();
          linePath.moveTo(xMid, (top + bottom) / 2);
        } else {
          linePath.lineTo(xMid, (top + bottom) / 2);
        }
      }
      iPrev = i;
      i = iNext;
    } else {
      // no downsampling
      if (!linePath) {
        linePath = new Path2D();
        linePath.moveTo(x, y);
      } else {
        linePath.lineTo(x, y);
      }
      i += 1;
    }
  }
  if (topEnvelope && bottomEnvelope) {
    envelopePaths.push(envelopeToPath(topEnvelope, bottomEnvelope, strokeWidth));
    topEnvelope = null;
    bottomEnvelope = null;
    linePath?.lineTo(floorX(idxToX(iEnd - 1)), wavToY(wav[iEnd - 1]));
  }
  if (doClear) {
    ctx.clearRect(0, 0, width, height);
  }
  if (needBorderForLine && linePath) {
    ctx.lineCap = "round";
    ctx.lineJoin = "round";
    ctx.strokeStyle = WAV_BORDER_COLOR;
    ctx.lineWidth = strokeWidth + 2 * WAV_BORDER_WIDTH * devicePixelRatio;
    ctx.stroke(linePath);
  }
  if (needBorderForEnvelope) {
    envelopePaths.forEach((path) => {
      ctx.lineCap = "round";
      ctx.lineJoin = "round";
      ctx.strokeStyle = WAV_BORDER_COLOR;
      ctx.lineWidth = 2 * WAV_BORDER_WIDTH * devicePixelRatio;
      ctx.stroke(path);
    });
  }

  if (linePath) {
    ctx.lineCap = "round";
    ctx.lineJoin = "round";
    ctx.strokeStyle = color;
    ctx.lineWidth = strokeWidth;
    ctx.stroke(linePath);
  }
  envelopePaths.forEach((path) => {
    ctx.fillStyle = color;
    ctx.fill(path);
  });
};
