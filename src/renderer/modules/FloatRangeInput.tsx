import React, {useRef, useEffect, forwardRef, useImperativeHandle} from "react";
import useEvent from "react-use-event-hook";
import {DEFAULT_RANGE_COLOR} from "renderer/prototypes/constants/colors";
import styles from "./FloatRangeInput.module.scss";

type FloatRangeInputProps = {
  id: string;
  className?: string;
  unit: string;
  min: number;
  max: number;
  step: number;
  precision: number;
  detents: number[];
  disabled?: boolean;
  initialValue: number;
  disabledValue?: number;
  doubleClickValue?: number | null;
  onChangeValue?: (value: number) => void;
};

const FloatRangeInput = forwardRef(
  (
    {
      className = "",
      disabled = false,
      disabledValue = NaN,
      doubleClickValue = null,
      onChangeValue = () => {},
      ...props
    }: FloatRangeInputProps,
    ref,
  ) => {
    const {id, unit, min, max, step, precision, initialValue, detents} = props;
    const rangeElem = useRef<HTMLInputElement>(null);
    const textElem = useRef<HTMLInputElement>(null);

    const getRangeBackground = () => {
      const rangeRatio = Math.min(
        Math.max((Number(textElem.current?.value ?? initialValue) - min) / (max - min), 0),
        1,
      );
      return disabled
        ? ""
        : `linear-gradient(to right, ${DEFAULT_RANGE_COLOR.LEFT} ${rangeRatio * 100}%, ${
            DEFAULT_RANGE_COLOR.RIGHT
          } ${rangeRatio * 100}%)`;
    };

    const getTextElemSize = () =>
      textElem.current?.value.length || initialValue.toFixed(precision).length;

    const updateStyle = useEvent(() => {
      if (rangeElem.current) {
        rangeElem.current.style.background = getRangeBackground();
      }
      if (textElem.current) {
        textElem.current.size = getTextElemSize();
      }
    });

    const setValue = useEvent((value: number) => {
      if (value >= min && value <= max) {
        if (rangeElem.current) rangeElem.current.value = value.toFixed(precision);
        if (textElem.current) textElem.current.value = value.toFixed(precision);
      }
      updateStyle();
    });

    const onRangeChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const value = Number.parseFloat(e.target.value);
      if (textElem.current) textElem.current.value = value.toFixed(precision);
      onChangeValue(value);
      updateStyle();
    };

    const onRangeDoubleClick = (e: React.MouseEvent<HTMLInputElement>) => {
      if (e.button === 0 && e.detail === 2 && doubleClickValue !== null) {
        if (rangeElem.current) rangeElem.current.value = doubleClickValue.toFixed(precision);
        if (textElem.current) textElem.current.value = doubleClickValue.toFixed(precision);
        onChangeValue(doubleClickValue);
        updateStyle();
      }
    };

    const onTextChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const value = Number.parseFloat(e.target.value);
      if (e.target.value !== "" && value >= min && value <= max) {
        if (rangeElem.current) rangeElem.current.value = value.toFixed(precision);
        onChangeValue(value);
        updateStyle();
      }
    };

    useEffect(() => {
      if (disabled) {
        const value = Number.isNaN(disabledValue) ? initialValue : disabledValue;
        if (rangeElem.current) rangeElem.current.value = value.toFixed(precision);
        if (textElem.current) textElem.current.value = value.toFixed(precision);
      }
    }, [disabled, initialValue, disabledValue, precision]);

    const imperativeInstanceRef = useRef<FloatRangeInputElement>({setValue});
    useImperativeHandle(ref, () => imperativeInstanceRef.current, []);

    return (
      <div className={`${styles.FloatRangeInput} ${className}`}>
        <input
          ref={rangeElem}
          id={id}
          style={{
            background: getRangeBackground(),
          }}
          type="range"
          min={min}
          max={max}
          step={step}
          defaultValue={initialValue}
          disabled={disabled}
          onChange={onRangeChange}
          onClick={onRangeDoubleClick}
          list={`${id}Detents`}
        />
        <datalist id={`${id}Detents`}>
          {detents.map((detent) => (
            <option
              key={`${id}Detents.value${detent}`}
              aria-label={`${id} detent value ${detent}`}
              value={detent}
            />
          ))}
        </datalist>
        <input
          ref={textElem}
          id={`${id}Text`}
          type="text"
          inputMode="decimal"
          size={getTextElemSize()}
          defaultValue={initialValue.toFixed(precision)}
          disabled={disabled}
          onChange={onTextChange}
        />
        <label htmlFor={`${id}Text`}>{unit}</label>
      </div>
    );
  },
);

FloatRangeInput.displayName = "FloatRangeInput";

export default React.memo(FloatRangeInput);
