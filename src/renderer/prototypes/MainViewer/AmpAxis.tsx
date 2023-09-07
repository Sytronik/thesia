import React, {forwardRef} from "react";
import AxisCanvas from "renderer/modules/AxisCanvas";
import {AMP_CANVAS_WIDTH, AMP_MARKER_POS, VERTICAL_AXIS_PADDING} from "../constants";

const AmpAxis = forwardRef(({height}: {height: number}, ref) => {
  return (
    <AxisCanvas
      ref={ref}
      width={AMP_CANVAS_WIDTH}
      height={height}
      axisPadding={VERTICAL_AXIS_PADDING}
      markerPos={AMP_MARKER_POS}
      direction="V"
      className="ampAxis"
    />
  );
});

AmpAxis.displayName = "AmpAxis";

export default AmpAxis;
