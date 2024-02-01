import {HexCode, HexCodeColorMap, RGBColorMap} from "renderer/utils/colorUtils";

/* Color System */
// must consistent with color-system.scss
export const BLACK: HexCode = "#000";
export const WHITE: HexCode = "#fff";
export const PRIMARY: HexCodeColorMap = {
  "400": "#858ef2",
  "500": "#666bcc",
  "600": "#4c4f75",
  "700": "#393b54",
  "750": "#30324a",
  "800": "#282a42",
  "900": "#1f2133",
};
export const PRIMARY_RGB: RGBColorMap = {
  "400": [133, 142, 242],
  "500": [102, 107, 204],
  "600": [76, 79, 117],
  "700": [57, 59, 84],
  "750": [48, 50, 74],
  "800": [40, 42, 66],
  "900": [31, 33, 51],
};
export const GRAY: HexCodeColorMap = {
  "400": "#b9b9b9",
  "500": "#707070",
  "600": "#5c5d73",
};
export const GRAY_RGB: RGBColorMap = {
  "400": [185, 185, 185],
  "500": [112, 112, 112],
  "600": [92, 93, 115],
};

export const BG_COLOR: HexCodeColorMap = {
  MAIN: PRIMARY[900],
  PRIMARY: PRIMARY[800],
  PRIMARY_SEMILIGHT: PRIMARY[750],
  PRIMARY_LIGHT: PRIMARY[700],
  PRIMARY_LIGHTER: PRIMARY[600],
};
export const BG_COLOR_RGB: RGBColorMap = {
  MAIN: PRIMARY_RGB[900],
  PRIMARY: PRIMARY_RGB[800],
  PRIMARY_SEMILIGHT: PRIMARY_RGB[750],
  PRIMARY_LIGHT: PRIMARY_RGB[700],
  PRIMARY_LIGHTER: PRIMARY_RGB[600],
};

export const BORDER_COLOR: HexCodeColorMap = {
  GRAY_LIGHT: GRAY[400],
  PRIMARY_LIGHT: PRIMARY[400],
};
export const BORDER_COLOR_RGB: RGBColorMap = {
  GRAY_LIGHT: GRAY_RGB[400],
  PRIMARY_LIGHT: PRIMARY_RGB[400],
};

/* Control */
export const BLEND_RANGE_COLOR = {LEFT: "#2D92E5", RIGHT: "#F59149"};
export const DEFAULT_RANGE_COLOR = {LEFT: PRIMARY[400], RIGHT: PRIMARY[600]};
