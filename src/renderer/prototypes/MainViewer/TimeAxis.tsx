import React, {forwardRef} from "react";
import AxisCanvas from "renderer/modules/AxisCanvas";
import {HORIZONTAL_AXIS_PADDING, TIME_CANVAS_HEIGHT, TIME_MARKER_POS} from "../constants";

const TimeAxis = forwardRef(
  ({width, shiftWhenResize}: {width: number; shiftWhenResize: boolean}, ref) => {
    return (
      <AxisCanvas
        ref={ref}
        width={width}
        height={TIME_CANVAS_HEIGHT}
        axisPadding={HORIZONTAL_AXIS_PADDING}
        markerPos={TIME_MARKER_POS}
        direction="H"
        className="timeRuler"
        shiftWhenResize={shiftWhenResize}
      />
    );
  },
);

TimeAxis.displayName = "TimeAxis";

export default TimeAxis;
