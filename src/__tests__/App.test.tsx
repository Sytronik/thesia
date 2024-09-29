import "@testing-library/jest-dom";
import {render} from "@testing-library/react";
import App from "../renderer/App";
import BackendAPI from "../renderer/api";

describe("App", () => {
  it("should render", () => {
    expect(render(<App userSettings={BackendAPI.init({})} />)).toBeTruthy();
  });
});
