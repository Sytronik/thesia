import React from "react";
import styles from "./TimeUnitSection.module.scss";

function TimeUnitSection(props: {timeUnitLabel: string}) {
  const {timeUnitLabel} = props;

  return (
    <div className={styles.timeUnitSection}>
      <p>{timeUnitLabel}</p>
    </div>
  );
}

export default TimeUnitSection;
