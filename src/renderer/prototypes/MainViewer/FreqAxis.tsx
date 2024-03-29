import React, {forwardRef} from "react";
import AxisCanvas from "renderer/modules/AxisCanvas";
import {FREQ_CANVAS_WIDTH, FREQ_MARKER_POS, VERTICAL_AXIS_PADDING} from "../constants/tracks";

const FreqAxis = forwardRef(({height}: {height: number}, ref) => {
  return (
    <AxisCanvas
      ref={ref}
      width={FREQ_CANVAS_WIDTH}
      height={height}
      axisPadding={VERTICAL_AXIS_PADDING}
      markerPos={FREQ_MARKER_POS}
      direction="V"
      className="freqAxis"
    />
  );
});

FreqAxis.displayName = "FreqAxis";

export default FreqAxis;
