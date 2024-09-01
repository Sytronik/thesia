import React, {useEffect, useRef} from "react";
import styles from "./FloatingUserInput.module.scss";

type FloatingUserInputProps = {
  value: string;
  onEndEditing: (v: string | null) => void;
  hidden: boolean;
  top: number;
  left: number;
};

function FloatingUserInput(props: FloatingUserInputProps) {
  const {value, onEndEditing: endEditingCallback, hidden, top, left} = props;
  const changedRef = useRef<boolean>(false);
  const inputElem = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!hidden && inputElem.current !== null && inputElem.current !== document.activeElement) {
      changedRef.current = false;
      inputElem.current.value = value;
      inputElem.current?.focus();
      inputElem.current?.select();
    }
  }, [value, hidden]);

  return (
    <input
      type="text"
      ref={inputElem}
      hidden={hidden}
      className={styles.floatingInput}
      style={{top, left}}
      defaultValue={value}
      tabIndex={-1}
      onMouseDown={(e) => {
        e.stopPropagation();
      }}
      onChange={() => {
        changedRef.current = true;
      }}
      onBlur={(e) => {
        endEditingCallback(changedRef.current ? (e.target as HTMLInputElement).value : null);
      }}
      onKeyDown={(e) => {
        const target = e.target as HTMLInputElement;
        switch (e.key) {
          case "Enter":
            target.blur();
            break;
          case "Escape":
            changedRef.current = false;
            target.blur();
            break;
          case "Tab":
            e.preventDefault();
            target.select();
            break;
          default:
            break;
        }
      }}
    />
  );
}

export default React.memo(FloatingUserInput);
