import React, {forwardRef} from "react";
import AxisCanvas from "renderer/modules/AxisCanvas";
import {TIME_CANVAS_HEIGHT, TIME_MARKER_POS} from "../constants";

const TimeAxis = forwardRef((props: {width: number}, ref) => {
  const {width} = props;
  return (
    <AxisCanvas
      ref={ref}
      width={width}
      height={TIME_CANVAS_HEIGHT}
      markerPos={TIME_MARKER_POS}
      direction="H"
      className="timeRuler"
    />
  );
});

TimeAxis.displayName = "TimeAxis";

export default TimeAxis;
