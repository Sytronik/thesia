import React from "react";
import styles from "./PlayerControl.module.scss";

function PlayerControl() {
  return <div className={`flex-item-fixed ${styles.PlayerControl}`} />;
}

export default React.memo(PlayerControl);
