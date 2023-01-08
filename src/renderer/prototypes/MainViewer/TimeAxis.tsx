import React, {forwardRef} from "react";
import AxisCanvas from "renderer/modules/AxisCanvas";
import {
  TIME_CANVAS_HEIGHT,
  TIME_MARKER_POS,
  AMP_CANVAS_WIDTH,
  FREQ_CANVAS_WIDTH,
} from "../constants";

const AXIS_SPACE = AMP_CANVAS_WIDTH + FREQ_CANVAS_WIDTH;

const TimeAxis = forwardRef((props: {width: number}, ref) => {
  const {width} = props;
  return (
    <AxisCanvas
      ref={ref}
      width={width - AXIS_SPACE}
      height={TIME_CANVAS_HEIGHT}
      markerPos={TIME_MARKER_POS}
      direction="H"
      className="timeRuler"
    />
  );
});

TimeAxis.displayName = "TimeAxis";

export default TimeAxis;
