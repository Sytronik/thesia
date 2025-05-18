import {getRGBA} from "renderer/utils/colorUtils";
import {WHITE, BG_COLOR_RGB, BORDER_COLOR} from "./colors";

// audio file
export const CHANNEL = [
  [], // Unreachable
  ["M"], // mono
  ["L", "R"], // stereo
];

export const SHIFT_PX = 40; // css px
export const BIG_SHIFT_PX = 200; // css px

// axis
const createBoundaries = (tickScaleTable: TickScaleTable) => {
  return Object.keys(tickScaleTable)
    .map((boundary) => Number(boundary))
    .sort((a, b) => b - a);
};

export const AXIS_STYLE = {
  LINE_WIDTH: 1,
  TICK_COLOR: WHITE,
  LABEL_COLOR: WHITE,
  LABEL_FONT: "11px sans-serif",
};

export const OVERVIEW_LENS_STYLE = {
  OUT_LENS_FILL_STYLE: getRGBA(BG_COLOR_RGB.MAIN, 0.6),
  LENS_STROKE_STYLE: BORDER_COLOR.GRAY_LIGHT,
  OUT_TRACK_FILL_STYLE: "rgba(0, 0, 0, 0.2)",
  LINE_WIDTH: 1.6,
  RESIZE_CURSOR: "col-resize",
};

export const TIME_CANVAS_HEIGHT = 16;
export const TIME_MARKER_POS = {
  MAJOR_TICK_POS: 2, // LENGTH: 14px, 16px - 14px
  MINOR_TICK_POS: 13, // LENGTH: 3px, 16px - 13px
  LABEL_POS: 2,
  LABEL_LEFT_MARGIN: 4,
};

export const TIME_TICK_SIZE: TickScaleTable = {
  // px per sec : [minor unit, number of subticks]
  0.00091: [3600, 5],
  0.00241: [3600, 2],
  0.00544: [600, 6],
  0.02444: [600, 3],
  0.03251: [600, 2],
  0.0611: [120, 5],
  0.12221: [60, 5],
  0.32507: [60, 2],
  0.73325: [10, 6],
  2.28: [10, 3],
  3.0324: [10, 2],
  5.7: [2, 5],
  11.4: [1, 5],
  30.324: [1, 2],
  57.0: [0.2, 5],
  136.375: [0.1, 5],
  362.757: [0.1, 2],
  681.875: [0.02, 5],
  1527.0: [0.01, 5],
  4061.0: [0.01, 2],
  7635.0: [0.002, 5],
  16902.5: [0.001, 5],
  44960.7: [0.001, 2],
  84512.5: [0.001, 1],
  159384: [0.0005, 2],
  234256: [0.0002, 5],
  309128: [0.0001, 10],
  // 384000 is max
  // 2016750: [0.00001, 5],
  // 5364555: [0.00001, 2],
  // 10083750: [0.000002, 5],
  // 21800000: [0.000001, 5],
};
export const TIME_BOUNDARIES = createBoundaries(TIME_TICK_SIZE);

export const LABEL_HEIGHT_ADJUSTMENT = 4;
export const AMP_CANVAS_WIDTH = 45;
export const AMP_MARKER_POS = {
  MAJOR_TICK_POS: 4,
  MINOR_TICK_POS: 3,
  LABEL_POS: 4,
  LABEL_LEFT_MARGIN: 3,
};
export const AMP_TICK_NUM: TickScaleTable = {
  // height: [max_number_of_ticks, max_number_of_labels]
  // TEMP
  80: [5, 5],
  230: [13, 13],
  300: [15, 15],
  305: [17, 17],
  320: [19, 19],
  340: [21, 21],
  405: [23, 23],
  480: [25, 25],
  495: [25, 25],
  505: [27, 27],
  530: [29, 29],
  560: [31, 31],
  590: [33, 33],
  610: [35, 35],
  635: [37, 37],
  660: [39, 39],
  740: [41, 41],
  765: [43, 43],
  790: [45, 45],
  835: [47, 47],
  920: [49, 49],
  940: [51, 51],
  985: [53, 53],
  995: [55, 55],
  1035: [57, 57],
  1070: [59, 59],
  1165: [61, 61],
  1185: [63, 63],
  1210: [65, 65],
  2000: [101, 101],
  3500: [203, 203],
};
export const AMP_BOUNDARIES = createBoundaries(AMP_TICK_NUM);

export const FREQ_CANVAS_WIDTH = 45;
export const FREQ_MARKER_POS = {
  MAJOR_TICK_POS: 4, // LENGTH: 4px
  MINOR_TICK_POS: 3, // LENGTH: 3px,
  LABEL_POS: 4, // same as MAJOR_TICK_POS
  LABEL_LEFT_MARGIN: 3,
};
export const FREQ_TICK_NUM: TickScaleTable = {
  // height: [max_number_of_ticks, max_number_of_labels]
  // TEMP
  80: [4, 2],
  90: [6, 3],
  100: [6, 4],
  120: [9, 5],
  150: [10, 6],
  200: [11, 7],
  240: [12, 8],
  280: [14, 9],
  320: [15, 10],
  360: [18, 12],
  400: [22, 14],
  450: [25, 16],
  500: [28, 18],
  600: [30, 20],
  700: [40, 25],
  850: [50, 30],
  1000: [60, 40],
  1500: [100, 60],
};
export const FREQ_BOUNDARIES = createBoundaries(FREQ_TICK_NUM);

export const TINY_MARGIN = 2;
// margin exist between amp axis and freq axis
export const AXIS_SPACE = AMP_CANVAS_WIDTH + FREQ_CANVAS_WIDTH + TINY_MARGIN;

export const COLORBAR_CANVAS_WIDTH = 16;
export const SLIDE_ICON_HEIGHT = 18;

export const DB_CANVAS_WIDTH = 32;
export const DB_MARKER_POS = {
  MAJOR_TICK_POS: 4,
  MINOR_TICK_POS: 3,
  LABEL_POS: 4,
  LABEL_LEFT_MARGIN: 3,
};
export const DB_TICK_NUM: TickScaleTable = {
  80: [4, 4],
  120: [6, 6],
  250: [13, 13],
  520: [25, 25],
  1000: [60, 60],
};
export const DB_BOUNDARIES = createBoundaries(DB_TICK_NUM);

export const MIN_TICK_SCALE_BOUNDARY = 80;
export const MIN_HEIGHT = MIN_TICK_SCALE_BOUNDARY + 73;
export const MAX_HEIGHT = 5000;

export const VERTICAL_AXIS_PADDING = 4;
export const HORIZONTAL_AXIS_PADDING = 0;

export const MAX_PX_PER_SEC = 384000;
export const FIT_TOLERANCE_SEC = 1e-6;

export const DEFAULT_AMP_RANGE: [number, number] = [-1, 1];
export const MIN_ABS_AMP_RANGE = 1e-5;
export const MAX_ABS_AMP_RANGE = 5;

export const MIN_COMMON_NORMALIZE_dB = -40;
export const COMMON_NORMALIZE_DB_DETENTS = [-26, -18, 0];

export const DB_RANGE_MIN_MAX = [40, 120];
export const DB_RANGE_DETENTS = [40, 70, 100, 120];

export const MIN_HZ_RANGE = 100;
export const MIN_DIST_FROM_0_FOR_DRAG = 0.01;

export const MIN_WIN_MILLISEC = 1.0;

export const T_OVERLAP_VALUES = [1, 2, 4, 8, 16, 32];

export const MIN_VOLUME_dB = -36;

export const WAV_MARGIN_RATIO = 0.1;
export const WAV_IMAGE_SCALE = 2;
export const WAV_LINE_WIDTH_FACTOR = 1.75;
export const WAV_BORDER_WIDTH = 1.5;
export const WAV_TOPBOTTOM_CONTEXT_SIZE = 2;

export const OVERVIEW_CH_GAP_HEIGHT = 1;
export const OVERVIEW_GAIN_HEIGHT_RATIO = 0.2;
export const OVERVIEW_LINE_WIDTH_FACTOR = 1;
