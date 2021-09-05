const PROPERTY = {
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
    MAJOR_TICK_POS: 4, // LENGTH: 12px, 16px - 12px
    MINOR_TICK_POS: 13, // LENGTH: 3px, 16px - 13px
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

  AMP_CANVAS_WIDTH: 23,
  AMP_MARKER_POS: {
    MAJOR_TICK_POS: 4,
    MINOR_TICK_POS: 3,
    LABEL_POS: 4,
    LABEL_LEFT_MARGIN: 3,
  },
  AMP_TICK_NUM: {
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
  },

  FREQ_CANVAS_WIDTH: 23, // 24px - 1px(border)
  FREQ_MARKER_POS: {
    MAJOR_TICK_POS: 4, // LENGTH: 4px
    MINOR_TICK_POS: 3, // LENGTH: 3px,
    LABEL_POS: 4, // same as MAJOR_TICK_POS
    LABEL_LEFT_MARGIN: 3,
  },
  FREQ_TICK_NUM: {
    // height: [max_number_of_ticks, max_number_of_labels]
    // TEMP
    80: [4, 2],
    230: [9, 7],
    305: [15, 10],
    330: [15, 11],
    375: [15, 12],
    445: [15, 13],
    515: [26, 18],
    560: [26, 19],
    620: [26, 20],
    635: [26, 21],
    690: [26, 22],
    720: [26, 23],
    765: [26, 24],
  },

  DB_CANVAS_WIDTH: 47, // 48px - 1px(border)
  DB_MARKER_POS: {
    MAJOR_TICK_POS: 4,
    MINOR_TICK_POS: 3,
    LABEL_POS: 4,
    LABEL_LEFT_MARGIN: 3,
  },
  DB_TICK_NUM: {
    80: [13, 13],
    350: [25, 25],
  },
};

export default PROPERTY;
