export type HexCode = `#${string}`;
export type HexCodeColorMap = {
  [key: string]: HexCode;
};
export type RGB = [number, number, number];
export type RGBColorMap = {
  [key: string]: RGB;
};

export function getRGBA(rgb: RGB, alpha: number) {
  const [r, g, b] = rgb;
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}
