import React, {ReactNode, useMemo, useRef, useState} from "react";
import useEvent from "react-use-event-hook";
import {debounce, throttle} from "throttle-debounce";
import {
  SpecSetting,
  GuardClippingMode,
  NormalizeTarget,
  FreqScale,
  NormalizeOnType,
  NormalizeOnTypeValues,
} from "renderer/api/backend-wrapper";
import FloatRangeInput from "renderer/modules/FloatRangeInput";
import styles from "./Control.module.scss";
import {
  COMMON_NORMALIZE_DB_DETENTS,
  DB_RANGE_DETENTS,
  DB_RANGE_MIN_MAX,
  MIN_COMMON_NORMALIZE_dB,
  T_OVERLAP_VALUES,
} from "../constants";

type ControlProps = {
  specSetting: SpecSetting;
  setSpecSetting: (specSetting: SpecSetting) => Promise<void>;
  dBRange: number;
  setdBRange: (dBRange: number) => void;
  commonGuardClipping: GuardClippingMode;
  setCommonGuardClipping: (commonGuardClipping: GuardClippingMode) => Promise<void>;
  commonNormalize: NormalizeTarget;
  setCommonNormalize: (commonNormalize: NormalizeTarget) => Promise<void>;
};

function Control(props: ControlProps) {
  const {
    specSetting,
    setSpecSetting,
    dBRange,
    setdBRange,
    commonGuardClipping,
    setCommonGuardClipping,
    commonNormalize,
    setCommonNormalize,
  } = props;

  const [cursorOnFreqScaleBtn, setCursorOnFreqScaleBtn] = useState<boolean>(false);
  const [commonNormalizePeakdB, setCommonNormalizePeakdB] = useState<number>(0.0);
  const [commonNormalizedB, setCommonNormalizedB] = useState<number>(-18.0);
  const [isCommonNormalizeOn, setIsCommonNormalizeOn] = useState<boolean>(false);

  const commonNormalizedBInputElem = useRef<FloatRangeInputElement>(null);

  const toggleFreqScale = useEvent((freqScale: FreqScale) =>
    freqScale === FreqScale.Linear ? FreqScale.Mel : FreqScale.Linear,
  );

  const onFreqScaleBtnClick = async () => {
    await setSpecSetting({
      ...specSetting,
      freqScale: toggleFreqScale(specSetting.freqScale),
    });
  };

  const throttledSetdBRange = useMemo(() => throttle(1000 / 70, setdBRange), [setdBRange]);

  const onWinMillisecChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const winMillisec = Number.parseFloat(e.target.value);
    if (e.target.value !== "" && winMillisec > 0) {
      setSpecSetting({
        ...specSetting,
        winMillisec,
      });
    }
  };

  const onTOverlapChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const tOverlap = Number.parseFloat(e.target.value);
    if (e.target.value !== "" && tOverlap > 0) {
      setSpecSetting({
        ...specSetting,
        tOverlap,
      });
    }
  };

  const onCommonNormalizeTypeChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const type = e.target.selectedOptions[0].value;
    switch (type) {
      case "Off":
        setCommonNormalize({type: "Off"});
        setIsCommonNormalizeOn(false);
        break;
      case "PeakdB":
        setCommonNormalize({type: "PeakdB", target: commonNormalizePeakdB});
        setIsCommonNormalizeOn(true);
        if (commonNormalizedBInputElem.current)
          commonNormalizedBInputElem.current.setValue(commonNormalizePeakdB);
        break;
      default:
        setCommonNormalize({type: type as NormalizeOnType, target: commonNormalizedB});
        setIsCommonNormalizeOn(true);
        if (commonNormalizedBInputElem.current)
          commonNormalizedBInputElem.current.setValue(commonNormalizedB);
        break;
    }
  };

  const debouncedChangeCommonNormalizedB = debounce(250, (dB: number) => {
    if (isCommonNormalizeOn) setCommonNormalize({type: commonNormalize.type, target: dB});
  });

  const onCommonNormalizedBInputChange = useEvent((value: number) => {
    if (!isCommonNormalizeOn) return;
    if (commonNormalize.type === "PeakdB") setCommonNormalizePeakdB(value);
    else setCommonNormalizedB(value);
    debouncedChangeCommonNormalizedB(value);
  });

  const onCommonGuardClippingModeChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    setCommonGuardClipping(e.target.selectedOptions[0].value as GuardClippingMode);
  };

  return (
    <div className={`flex-item-fixed ${styles.Control}`}>
      <div className={styles.sectionContainer}>
        <div className={styles.itemContainer}>
          <label htmlFor="dBRange">Dynamic Range</label>
          <FloatRangeInput
            id="dBRange"
            unit="dB"
            min={DB_RANGE_MIN_MAX[0]}
            max={DB_RANGE_MIN_MAX[1]}
            step={1}
            precision={0}
            detents={DB_RANGE_DETENTS}
            initialValue={dBRange}
            doubleClickValue={DB_RANGE_MIN_MAX[1]}
            onChangeValue={throttledSetdBRange}
          />
        </div>
      </div>
      <div className={styles.sectionContainer}>
        <div className={styles.itemContainer}>
          <label htmlFor="winMillisec">Window Size</label>
          <input
            type="text"
            inputMode="decimal"
            id="winMillisec"
            className={styles.winMillisecInput}
            defaultValue={specSetting.winMillisec.toFixed(1)}
            onChange={onWinMillisecChange}
          />
          ms
        </div>
        <div className={styles.itemContainer}>
          <label htmlFor="tOverlap">Time Overlap</label>
          <select
            name="tOverlap"
            id="tOverlap"
            defaultValue={specSetting.tOverlap}
            onChange={onTOverlapChange}
          >
            {T_OVERLAP_VALUES.map((v) => (
              <option key={`t-overlap-${v}x`} value={`${v}`}>{`${v}x`}</option>
            ))}
          </select>
        </div>
      </div>
      <div className={styles.sectionContainer}>
        <div className={styles.itemContainer}>
          <label htmlFor="freqScale">Frequency Scale</label>
          <input
            type="button"
            className={styles.changeFreqScaleBtn}
            onClick={onFreqScaleBtnClick}
            onMouseEnter={() => setCursorOnFreqScaleBtn(true)}
            onMouseLeave={() => setCursorOnFreqScaleBtn(false)}
            defaultValue={
              cursorOnFreqScaleBtn
                ? `to ${toggleFreqScale(specSetting.freqScale)}`
                : specSetting.freqScale
            }
            id="freqScale"
          />
        </div>
      </div>
      <div className={styles.sectionContainer}>
        <div className={styles.itemContainer}>
          <label htmlFor="commonNormalize">Common Normalization</label>
          <select
            name="commonNormalize"
            id="commonNormalize"
            onChange={onCommonNormalizeTypeChange}
            defaultValue={commonNormalize.type}
          >
            <option value="Off">Off</option>
            {NormalizeOnTypeValues.map((type) => (
              <option key={type} value={type}>
                {type.replace("dB", "")}
              </option>
            ))}
          </select>
          <FloatRangeInput
            ref={commonNormalizedBInputElem}
            id="commonNormalizedBInput"
            unit="dB"
            min={MIN_COMMON_NORMALIZE_dB}
            max={0}
            step={0.01}
            precision={2}
            detents={COMMON_NORMALIZE_DB_DETENTS}
            initialValue={MIN_COMMON_NORMALIZE_dB}
            disabled={!isCommonNormalizeOn}
            onChangeValue={onCommonNormalizedBInputChange}
          />
        </div>
        <div className={styles.itemContainer}>
          <label htmlFor="commonGuardClipping">Common Clipping Guard</label>
          <select
            name="commonGuardClipping"
            id="commonGuardClipping"
            onChange={onCommonGuardClippingModeChange}
            defaultValue={commonGuardClipping}
          >
            <option value={GuardClippingMode.ReduceGlobalLevel}>Reducing Global Level</option>
            <option value={GuardClippingMode.Limiter}>Applying Limiter</option>
            <option value={GuardClippingMode.Clip}>Off (Hard Clipping)</option>
          </select>
        </div>
      </div>
    </div>
  );
}

export default Control;
