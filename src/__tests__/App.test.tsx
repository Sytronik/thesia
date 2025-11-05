import "@testing-library/jest-dom";
import {render} from "@testing-library/react";
import App from "../renderer/App";
import BackendAPI from "../renderer/api";

describe("App", () => {
  it("should render", () => {
    const canvas = document.createElement("canvas");
    const gl = canvas.getContext("webgl2");
    if (!gl) throw new Error("WebGL2 is not supported");
    expect(
      render(<App userSettings={BackendAPI.init({}, gl.getParameter(gl.MAX_TEXTURE_SIZE), "")} />),
    ).toBeTruthy();
  });
});
