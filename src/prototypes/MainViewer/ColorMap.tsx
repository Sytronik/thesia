import React, { useRef, useEffect } from "react";
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
  const measuredHeight = useRef<number | null>(null);

  useEffect(() => {
    const colorMap = colorMapElem.current;
    if (!colorMap) return;
    let requestId: number | null = null;
    const resizeObserver = new ResizeObserver((entries) => {
      const { target } = entries[0];
      const nextHeight = Math.max(target.clientHeight - (16 + 2 + 24), MIN_HEIGHT);
      if (measuredHeight.current === nextHeight) return;
      measuredHeight.current = nextHeight;
      if (requestId !== null) cancelAnimationFrame(requestId);
      requestId = requestAnimationFrame(() => {
        requestId = null;
        setHeight(nextHeight);
      });
    });
    resizeObserver.observe(colorMap);

    return () => {
      if (requestId !== null) cancelAnimationFrame(requestId);
      resizeObserver.disconnect();
    };
  }, [setHeight]);

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
