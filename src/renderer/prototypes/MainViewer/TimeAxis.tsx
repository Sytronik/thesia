import React, {forwardRef} from "react";
import AxisCanvas from "renderer/modules/AxisCanvas";
import {HORIZONTAL_AXIS_PADDING, TIME_CANVAS_HEIGHT, TIME_MARKER_POS} from "../constants";

const TimeAxis = forwardRef((props: {width: number; pixelRatio: number}, ref) => {
  const {width, pixelRatio} = props;
  return (
    <AxisCanvas
      ref={ref}
      width={width}
      height={TIME_CANVAS_HEIGHT}
      pixelRatio={pixelRatio}
      axisPadding={HORIZONTAL_AXIS_PADDING}
      markerPos={TIME_MARKER_POS}
      direction="H"
      className="timeRuler"
    />
  );
});

TimeAxis.displayName = "TimeAxis";

export default TimeAxis;
