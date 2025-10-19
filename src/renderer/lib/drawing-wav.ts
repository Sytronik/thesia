import {WAV_BORDER_COLOR} from "renderer/prototypes/constants/colors";
import {WAV_BORDER_WIDTH, WAV_LINE_WIDTH_FACTOR} from "renderer/prototypes/constants/tracks";

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
