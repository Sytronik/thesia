import React, {forwardRef} from "react";
import AxisCanvas from "renderer/modules/AxisCanvas";
import {AMP_CANVAS_WIDTH, AMP_MARKER_POS, VERTICAL_AXIS_PADDING} from "../constants";

const AmpAxis = forwardRef((props: {height: number; pixelRatio: number}, ref) => {
  const {height, pixelRatio} = props;
  return (
    <AxisCanvas
      ref={ref}
      width={AMP_CANVAS_WIDTH}
      height={height}
      pixelRatio={pixelRatio}
      axisPadding={VERTICAL_AXIS_PADDING}
      markerPos={AMP_MARKER_POS}
      direction="V"
      noClearRect
      className="ampAxis"
    />
  );
});

AmpAxis.displayName = "AmpAxis";

export default AmpAxis;
