import React, {forwardRef, useCallback, useRef} from "react";
import AxisCanvas from "renderer/modules/AxisCanvas";
import styles from "renderer/modules/AxisCanvas.scss";
import {AMP_CANVAS_WIDTH, AMP_MARKER_POS, VERTICAL_AXIS_PADDING} from "../constants";

type AmpAxisProps = {
  height: number;
  onWheel: (e: WheelEvent) => void;
  onClick: (e: MouseEvent) => void;
};

const AmpAxis = forwardRef((props: AmpAxisProps, ref) => {
  const {height, onWheel, onClick} = props;
  const wrapperDivElem = useRef<HTMLDivElement | null>(null);

  const wrapperDivElemCallback = useCallback(
    (elem: HTMLDivElement) => {
      if (!elem) {
        wrapperDivElem.current?.removeEventListener("wheel", onWheel);
        wrapperDivElem.current?.removeEventListener("click", onClick);
        wrapperDivElem.current = null;
        return;
      }
      elem.addEventListener("wheel", onWheel, {passive: false});
      elem.addEventListener("click", onClick);
      wrapperDivElem.current = elem;
    },
    [onWheel, onClick],
  );

  return (
    <div ref={wrapperDivElemCallback} className={styles.ampAxisWrapper}>
      <AxisCanvas
        ref={ref}
        width={AMP_CANVAS_WIDTH}
        height={height}
        axisPadding={VERTICAL_AXIS_PADDING}
        markerPos={AMP_MARKER_POS}
        direction="V"
        className="ampAxis"
      />
    </div>
  );
});

AmpAxis.displayName = "AmpAxis";

export default AmpAxis;
