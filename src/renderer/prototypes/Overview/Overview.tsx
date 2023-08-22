import React, {
  useRef,
  useEffect,
  useState,
  useCallback,
  useMemo,
  forwardRef,
  useImperativeHandle,
} from "react";
import styles from "./Overview.scss";
import NativeAPI from "../../api";
import {OVERVIEW_LENS_STYLE} from "../constants";

const {ALPHA, LINE_WIDTH} = OVERVIEW_LENS_STYLE;

type OverviewProps = {
  selectedTrackId: number | null;
  pixelRatio: number;
};

type ArgsGetOverview = [trackId: number, width: number, height: number];
type ArgsLens = [startSec: number, lensDurationSec: number];

const Overview = forwardRef((props: OverviewProps, ref) => {
  const {selectedTrackId, pixelRatio} = props;
  const [resizeObserver, setResizeObserver] = useState(new ResizeObserver(() => {}));
  const durationSec = useMemo(
    () => (selectedTrackId !== null ? NativeAPI.getLength(selectedTrackId) : 0),
    [selectedTrackId],
  );

  const backgroundElem = useRef<HTMLCanvasElement>(null);
  const lensElem = useRef<HTMLCanvasElement>(null);
  const lensCtxRef = useRef<CanvasRenderingContext2D | null>(null);
  const prevArgsRef = useRef<ArgsGetOverview | null>(null);
  const prevArgsLensRef = useRef<ArgsLens | null>(null);

  const drawLens = useCallback(
    (startSec, lensDurationSec) => {
      const ctx = lensCtxRef.current;
      if (!lensElem.current || !ctx) return;
      const {clientWidth: width, clientHeight: height} = lensElem.current;
      const pxPerSec = width / durationSec;
      const endSec = (startSec + lensDurationSec) * pxPerSec;
      ctx.globalAlpha = ALPHA;
      ctx.lineWidth = LINE_WIDTH;
      ctx.clearRect(0, 0, width, height);
      if (startSec > 0) ctx.fillRect(0, 0, startSec * pxPerSec, height);
      ctx.strokeRect(
        startSec * pxPerSec + LINE_WIDTH / 2,
        LINE_WIDTH / 2,
        lensDurationSec * pxPerSec - LINE_WIDTH,
        height - LINE_WIDTH,
      );
      if (width > endSec) ctx.fillRect(endSec, 0, width - endSec, height);
      prevArgsLensRef.current = [startSec, lensDurationSec];
    },
    [durationSec],
  );

  const draw = useCallback(
    async (startSec: number, lensDurationSec: number, forced = false) => {
      if (!backgroundElem.current || !lensElem.current) return;

      const backgroundCtx = backgroundElem.current.getContext("bitmaprenderer");

      if (selectedTrackId === null) {
        if (forced || prevArgsRef.current !== null) {
          backgroundCtx?.transferFromImageBitmap(null);
          lensCtxRef.current?.clearRect(
            0,
            0,
            lensElem.current.clientWidth,
            lensElem.current.clientHeight,
          );
        }
        prevArgsRef.current = null;
        prevArgsLensRef.current = null;
        return;
      }

      if (!backgroundCtx) return;

      const {width, height} = backgroundElem.current;
      const args: ArgsGetOverview = [selectedTrackId, width, height];
      if (
        forced ||
        prevArgsRef.current === null ||
        prevArgsRef.current.some((v, i) => args[i] !== v)
      ) {
        const buf = await NativeAPI.getOverview(selectedTrackId, width, height);
        const imdata = new ImageData(new Uint8ClampedArray(buf), width, height);
        const imbmp = await createImageBitmap(imdata);
        backgroundCtx.transferFromImageBitmap(imbmp);
        prevArgsRef.current = args;
      }
      if (
        forced ||
        prevArgsLensRef.current === null ||
        prevArgsLensRef.current[0] !== startSec ||
        prevArgsLensRef.current[1] !== lensDurationSec
      ) {
        drawLens(startSec, lensDurationSec);
      }
    },
    [selectedTrackId, drawLens],
  );

  useEffect(() => {
    setResizeObserver(
      new ResizeObserver((entries) => {
        const backgroundElemTarget = entries[0].target as HTMLCanvasElement;
        backgroundElemTarget.width = backgroundElemTarget.clientWidth * pixelRatio;
        backgroundElemTarget.height = backgroundElemTarget.clientHeight * pixelRatio;
        if (!lensElem.current) return;
        lensElem.current.width = backgroundElemTarget.width;
        lensElem.current.height = backgroundElemTarget.height;
        lensCtxRef.current = lensElem.current.getContext("2d");
        lensCtxRef.current?.scale(pixelRatio, pixelRatio);
        if (prevArgsLensRef.current)
          draw(prevArgsLensRef.current[0], prevArgsLensRef.current[1], true);
      }),
    );
  }, [draw, pixelRatio]);

  useEffect(() => {
    if (backgroundElem.current) {
      resizeObserver.observe(backgroundElem.current);
    }

    return () => {
      resizeObserver.disconnect();
    };
  }, [resizeObserver]);

  useImperativeHandle(ref, () => ({draw}), [draw]);

  return (
    <div className={styles.Overview}>
      <canvas ref={backgroundElem} />
      <canvas ref={lensElem} />
    </div>
  );
});
Overview.displayName = "Overview";

export default Overview;
