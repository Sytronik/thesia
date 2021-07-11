export const PROPERTY = {
  // audio file
  CHANNEL: {
    1: ["M"],
    2: ["L", "R"],
  },
  SUPPORTED_TYPES: ["flac", "mp3", "oga", "ogg", "wav"],

  // axis
  AXIS_STYLE: {
    LINE_WIDTH: "1px",
    TICK_COLOR: "#fff",
    LABEL_COLOR: "#fff",
    LABEL_FONT: "11px sans-serif",
  },

  TIME_CANVAS_HEIGHT: 15, // 16px - 1px(border)
  TIME_MARKER_POS: {
    MAJOR_TICK_POS: 4, // LENGTH: 12px,
    MINOR_TICK_POS: 13, // LENGTH: 3px,
    LABEL_POS: 4,
    LABEL_LEFT_MARGIN: 4,
  },

  TIME_TICK_SIZE: {
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
    84512.5: [0.0002, 5],
    185350: [0.0001, 5],
    493031: [0.0001, 2],
    926750: [0.00002, 5],
    2016750: [0.00001, 5],
    5364555: [0.00001, 2],
    10083750: [0.000002, 5],
    21800000: [0.000001, 5],
  },
};
