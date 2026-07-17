import { VERTICAL_AXIS_PADDING } from "../prototypes/constants/tracks";

export const getAxisHeight = (rect: DOMRect) => rect.height - 2 * VERTICAL_AXIS_PADDING;
export const getAxisPos = (pos: number) => pos - VERTICAL_AXIS_PADDING;
