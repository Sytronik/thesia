import React, {useRef, useEffect, useState} from "react";
import {observer} from "mobx-react-lite";
import AxisCanvas from "renderer/modules/AxisCanvas";
import ColorBarCanvas from "renderer/prototypes/MainViewer/ColorBarCanvas";
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
  dBAxisCanvasElem: React.RefObject<AxisCanvasHandleElement>;
};

function ColorMap(props: ColorMapProps) {
  const {height, setHeight, colorBarHeight, dBAxisCanvasElem} = props;

  const colorMapElem = useRef<HTMLDivElement>(null);

  const [resizeObserver, _setResizeObserver] = useState(
    new ResizeObserver((entries) => {
      const {target} = entries[0];
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
          ref={dBAxisCanvasElem}
          width={DB_CANVAS_WIDTH}
          height={height}
          axisPadding={VERTICAL_AXIS_PADDING}
          markerPos={DB_MARKER_POS}
          direction="V"
          className="dBAxis"
        />
      </div>
    </div>
  );
}

export default observer(ColorMap);
