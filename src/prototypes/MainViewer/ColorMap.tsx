import React, { useRef, useEffect, useState } from "react";
import AxisCanvas from "src/modules/AxisCanvas";
import ColorBarCanvas from "src/prototypes/MainViewer/ColorBarCanvas";
import styles from "./ColorMap.module.scss";
import {
  COLORBAR_CANVAS_WIDTH,
  DB_CANVAS_WIDTH,
  DB_MARKER_POS,
  MIN_HEIGHT,
  VERTICAL_AXIS_PADDING,
} from "../constants/tracks";

type ColorMapProps = {
  height: number;
  setHeight: (height: number) => void;
  colorBarHeight: number;
  markersAndLength: [Markers, number];
};

function ColorMap(props: ColorMapProps) {
  const { height, setHeight, colorBarHeight, markersAndLength } = props;

  const colorMapElem = useRef<HTMLDivElement>(null);

  const [resizeObserver, _setResizeObserver] = useState(
    new ResizeObserver((entries) => {
      const { target } = entries[0];
      // Need throttle?
      setHeight(Math.max(target.clientHeight - (16 + 2 + 24), MIN_HEIGHT));
    }),
  );

  useEffect(() => {
    if (colorMapElem.current) {
      resizeObserver.observe(colorMapElem.current);
    }

    return () => {
      resizeObserver.disconnect();
    };
  }, [resizeObserver]);

  return (
    <div className={styles.colorMap} ref={colorMapElem}>
      <div className={styles.colorMapHeader}>dB</div>
      <div className={styles.colorMapBody}>
        <ColorBarCanvas width={COLORBAR_CANVAS_WIDTH} height={colorBarHeight} />
        <AxisCanvas
          id={0}
          width={DB_CANVAS_WIDTH}
          height={height}
          axisPadding={VERTICAL_AXIS_PADDING}
          markerPos={DB_MARKER_POS}
          markersAndLength={markersAndLength}
          direction="V"
          className="dBAxis"
          endInclusive
        />
      </div>
    </div>
  );
}

export default React.memo(ColorMap);
