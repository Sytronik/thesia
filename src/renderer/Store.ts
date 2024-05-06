import {makeAutoObservable} from "mobx";
import {useDevicePixelRatio} from "use-device-pixel-ratio";
import {VERTICAL_AXIS_PADDING} from "./prototypes/constants/tracks";

export default class Store {
  private width: number;
  private height: number;
  private imgHeight: number;
  private devicePixelRatio: number;

  constructor() {
    makeAutoObservable(this);
    this.width = 600;
    this.height = 250;
    this.imgHeight = 250 - 2 * VERTICAL_AXIS_PADDING;
    this.devicePixelRatio = useDevicePixelRatio(); // eslint-disable-line react-hooks/rules-of-hooks
  }

  public getWidth(): number {
    return this.width;
  }
  public getHeight(): number {
    return this.height;
  }
  public getImgHeight(): number {
    return this.imgHeight;
  }
  public getDPR(): number {
    return this.devicePixelRatio;
  }

  public setWidth(w: number): void {
    this.width = w;
  }
  public setHeight(h: number): void {
    this.height = h;
    this.imgHeight = h - 2 * VERTICAL_AXIS_PADDING;
  }
}
